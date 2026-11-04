pub mod subtitles;

use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use opentelemetry::KeyValue;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::process::Command;
use tokio::sync::Notify;
use tokio::task::{AbortHandle, JoinHandle};
use tracing::Instrument;

use crate::analysis::{Analysis, run_analyze_with};
use crate::config::Config;
use crate::error::ApiError;

const DONE_FILE: &str = "done.json";
const QUEUE_FULL_RETRY_SECS: u64 = 30;
const TTL_SWEEP_INTERVAL: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Interactive,
    Background,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobSpec {
    Analyze { path: String },
    Subtitles { path: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobRequest {
    pub job_id: String,
    pub priority: Priority,
    #[serde(flatten)]
    pub spec: JobSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JobStatus {
    pub state: JobState,
    pub progress: Option<f64>,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("{0}")]
    Analysis(#[from] crate::analysis::AnalysisError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg failed on stream {index}: {stderr}")]
    Ffmpeg { index: u32, stderr: String },
    #[error("cannot encode result: {0}")]
    Json(#[from] serde_json::Error),
}

/// Call before setting stdio or `kill_on_drop`: on unix the command is rebuilt around `nice`, which keeps only the program, arguments, working directory and environment.
pub fn lower_priority(cmd: &mut Command) {
    #[cfg(unix)]
    {
        let std = cmd.as_std();
        let program = std.get_program().to_owned();
        let args: Vec<OsString> = std.get_args().map(ToOwned::to_owned).collect();
        let mut nice = Command::new("nice");
        nice.args(["-n", "10"]).arg(program).args(args);
        if let Some(dir) = std.get_current_dir() {
            nice.current_dir(dir);
        }
        for (k, v) in std.get_envs() {
            match v {
                Some(v) => nice.env(k, v),
                None => nice.env_remove(k),
            };
        }
        *cmd = nice;
    }
    #[cfg(windows)]
    {
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        cmd.creation_flags(BELOW_NORMAL_PRIORITY_CLASS);
    }
}

struct Entry {
    priority: Priority,
    spec: JobSpec,
    state: JobState,
    result: Option<Value>,
    error: Option<String>,
    abort: Option<AbortHandle>,
}

#[derive(Default)]
struct Queue {
    jobs: HashMap<String, Entry>,
    interactive: VecDeque<String>,
    background: VecDeque<String>,
}

impl Queue {
    fn queued(&self) -> usize {
        self.interactive.len() + self.background.len()
    }
}

struct Shared {
    cfg: Arc<Config>,
    queue: Mutex<Queue>,
    wake: Notify,
}

#[derive(Clone)]
pub struct Jobs(Arc<Shared>);

fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn check_id(id: &str) -> Result<(), ApiError> {
    if valid_id(id) {
        Ok(())
    } else {
        Err(ApiError::invalid_request(
            "id must be 32 lowercase hex characters",
        ))
    }
}

fn not_found() -> ApiError {
    ApiError::not_found("no such job")
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn dir(&self, id: &str) -> PathBuf {
        self.cfg.jobs_dir.join(id)
    }

    fn take(&self) -> Option<(String, JobSpec)> {
        let mut q = self.lock();
        loop {
            let id = q
                .interactive
                .pop_front()
                .or_else(|| q.background.pop_front())?;
            let dir = self.dir(&id);
            let _ = std::fs::remove_dir_all(&dir);
            let entry = q.jobs.get_mut(&id)?;
            if std::fs::create_dir_all(&dir).is_ok() {
                entry.state = JobState::Running;
                return Some((id, entry.spec.clone()));
            }
            entry.state = JobState::Failed;
            entry.error = Some("cannot create the job directory".into());
        }
    }

    async fn run(&self, id: String, spec: JobSpec) {
        let cfg = self.cfg.clone();
        let dir = self.dir(&id);
        let kind = match &spec {
            JobSpec::Analyze { .. } => "analyze",
            JobSpec::Subtitles { .. } => "subtitles",
        };
        let span = tracing::info_span!("engine.job", job = %id, r#type = kind);
        let started = std::time::Instant::now();
        let task: JoinHandle<Result<Value, JobError>> =
            tokio::spawn(async move { execute(&cfg, &spec, &dir).await }.instrument(span));
        {
            let mut q = self.lock();
            match q.jobs.get_mut(&id) {
                Some(e) => e.abort = Some(task.abort_handle()),
                None => {
                    task.abort();
                    return;
                }
            }
        }
        let outcome = match task.await {
            Ok(r) => r.map_err(|e| e.to_string()),
            Err(e) if e.is_cancelled() => return,
            Err(_) => Err("job panicked".to_owned()),
        };
        let m = crate::telemetry::metrics();
        let outcome_label = if outcome.is_ok() { "done" } else { "failed" };
        m.jobs.add(
            1,
            &[
                KeyValue::new("type", kind),
                KeyValue::new("outcome", outcome_label),
            ],
        );
        m.job_duration.record(
            started.elapsed().as_secs_f64(),
            &[KeyValue::new("type", kind)],
        );
        let mut q = self.lock();
        let Some(entry) = q.jobs.get_mut(&id) else {
            return;
        };
        entry.abort = None;
        match outcome {
            Ok(v) => {
                entry.state = JobState::Done;
                entry.result = Some(v);
            }
            Err(msg) => {
                entry.state = JobState::Failed;
                entry.error = Some(msg);
            }
        }
        let record = json!({
            "state": if entry.state == JobState::Done { "done" } else { "failed" },
            "result": entry.result,
            "error": entry.error,
        });
        let _ = std::fs::write(self.dir(&id).join(DONE_FILE), record.to_string());
    }

    fn read_record(&self, id: &str) -> Option<JobStatus> {
        let text = std::fs::read_to_string(self.dir(id).join(DONE_FILE)).ok()?;
        let v: Value = serde_json::from_str(&text).ok()?;
        let state = match v["state"].as_str()? {
            "done" => JobState::Done,
            "failed" => JobState::Failed,
            _ => return None,
        };
        Some(JobStatus {
            state,
            progress: (state == JobState::Done).then_some(1.0),
            result: Some(v["result"].clone()).filter(|r| !r.is_null()),
            error: v["error"].as_str().map(ToOwned::to_owned),
        })
    }

    fn sweep_ttl(&self) {
        let Ok(entries) = std::fs::read_dir(&self.cfg.jobs_dir) else {
            return;
        };
        let now = SystemTime::now();
        for entry in entries.flatten() {
            let done = entry.path().join(DONE_FILE);
            let Ok(age) = std::fs::metadata(&done)
                .and_then(|m| m.modified())
                .map(|t| now.duration_since(t).unwrap_or_default())
            else {
                continue;
            };
            if age > self.cfg.job_output_ttl {
                let Some(id) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                    continue;
                };
                let mut q = self.lock();
                if q.jobs
                    .get(&id)
                    .is_none_or(|e| matches!(e.state, JobState::Done | JobState::Failed))
                {
                    q.jobs.remove(&id);
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
    }
}

async fn analyze(cfg: &Config, path: &str) -> Result<Analysis, JobError> {
    let input = cfg.path_map.rewrite(path);
    Ok(run_analyze_with(&cfg.ffprobe(), &input, &lower_priority).await?)
}

async fn execute(cfg: &Config, spec: &JobSpec, dir: &Path) -> Result<Value, JobError> {
    match spec {
        JobSpec::Analyze { path } => Ok(serde_json::to_value(analyze(cfg, path).await?)?),
        JobSpec::Subtitles { path } => {
            let analysis = analyze(cfg, path).await?;
            subtitles::run(&cfg.ffmpeg(), &cfg.path_map.rewrite(path), &analysis, dir).await
        }
    }
}

async fn worker(shared: Arc<Shared>) {
    loop {
        match shared.take() {
            Some((id, spec)) => shared.run(id, spec).await,
            None => shared.wake.notified().await,
        }
    }
}

fn name_ok(name: &str) -> bool {
    !name.contains('\\')
        && name.split('/').all(|seg| !seg.is_empty())
        && Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

impl Jobs {
    pub fn new(cfg: Arc<Config>) -> Jobs {
        let shared = Arc::new(Shared {
            cfg,
            queue: Mutex::new(Queue::default()),
            wake: Notify::new(),
        });
        for _ in 0..shared.cfg.max_jobs.max(1) {
            tokio::spawn(worker(shared.clone()));
        }
        Jobs(shared)
    }

    pub fn submit(&self, r: JobRequest) -> Result<(), ApiError> {
        check_id(&r.job_id)?;
        let mut q = self.0.lock();
        if q.jobs.contains_key(&r.job_id) || self.0.dir(&r.job_id).join(DONE_FILE).exists() {
            return Err(ApiError::job_exists());
        }
        if q.queued() >= self.0.cfg.max_queued {
            return Err(ApiError::queue_full(QUEUE_FULL_RETRY_SECS));
        }
        match r.priority {
            Priority::Interactive => q.interactive.push_back(r.job_id.clone()),
            Priority::Background => q.background.push_back(r.job_id.clone()),
        }
        q.jobs.insert(
            r.job_id,
            Entry {
                priority: r.priority,
                spec: r.spec,
                state: JobState::Queued,
                result: None,
                error: None,
                abort: None,
            },
        );
        drop(q);
        self.0.wake.notify_one();
        Ok(())
    }

    pub fn status(&self, id: &str) -> Result<JobStatus, ApiError> {
        check_id(id)?;
        if let Some(e) = self.0.lock().jobs.get(id) {
            return Ok(JobStatus {
                state: e.state,
                progress: (e.state == JobState::Done).then_some(1.0),
                result: e.result.clone(),
                error: e.error.clone(),
            });
        }
        self.0.read_record(id).ok_or_else(not_found)
    }

    pub fn file(&self, id: &str, name: &str) -> Result<PathBuf, ApiError> {
        if self.status(id)?.state != JobState::Done || !name_ok(name) || name == DONE_FILE {
            return Err(ApiError::not_found("no such file"));
        }
        let path = self.0.dir(id).join(name);
        if path.is_file() {
            Ok(path)
        } else {
            Err(ApiError::not_found("no such file"))
        }
    }

    pub fn download_path(&self, id: &str) -> Result<PathBuf, ApiError> {
        if self.status(id)?.state != JobState::Done {
            return Err(not_found());
        }
        let dir = self.0.dir(id);
        let mp4 = dir.join("output.mp4");
        if mp4.is_file() {
            return Ok(mp4);
        }
        std::fs::read_to_string(dir.join("original"))
            .map(|p| self.0.cfg.path_map.rewrite(p.trim()))
            .map_err(|_| not_found())
    }

    pub fn cancel(&self, id: &str) -> Result<(), ApiError> {
        check_id(id)?;
        let mut q = self.0.lock();
        let dir = self.0.dir(id);
        match q.jobs.remove(id) {
            Some(entry) => {
                if let Some(h) = entry.abort {
                    h.abort();
                }
                q.interactive.retain(|j| j != id);
                q.background.retain(|j| j != id);
            }
            None if dir.join(DONE_FILE).exists() => {}
            None => return Err(not_found()),
        }
        let _ = std::fs::remove_dir_all(dir);
        Ok(())
    }

    /// Running jobs first, then the queue in the order workers will take it.
    pub fn list(&self) -> Vec<(String, JobState, Priority)> {
        let q = self.0.lock();
        let mut running: Vec<_> = q
            .jobs
            .iter()
            .filter(|(_, e)| e.state == JobState::Running)
            .map(|(id, e)| (id.clone(), e.state, e.priority))
            .collect();
        running.sort_by(|a, b| a.0.cmp(&b.0));
        let queued = q.interactive.iter().chain(&q.background).filter_map(|id| {
            let e = q.jobs.get(id)?;
            Some((id.clone(), e.state, e.priority))
        });
        running.into_iter().chain(queued).collect()
    }

    pub fn sweep_boot(&self) {
        let Ok(entries) = std::fs::read_dir(&self.0.cfg.jobs_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_str().is_some_and(valid_id)
                && path.is_dir()
                && !path.join(DONE_FILE).exists()
            {
                let _ = std::fs::remove_dir_all(path);
            }
        }
    }

    pub fn sweep_ttl_once(&self) {
        self.0.sweep_ttl();
    }

    /// Stops running jobs, killing their ffmpeg children, and drops the queue.
    pub fn shutdown(&self) {
        let mut q = self.0.lock();
        q.interactive.clear();
        q.background.clear();
        for entry in q.jobs.values_mut() {
            if let Some(h) = entry.abort.take() {
                h.abort();
            }
        }
    }

    pub fn spawn_ttl_sweeper(&self) -> JoinHandle<()> {
        let shared = self.0.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(TTL_SWEEP_INTERVAL);
            loop {
                tick.tick().await;
                shared.sweep_ttl();
            }
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct Fixture {
        root: PathBuf,
        cfg: Config,
    }

    impl Fixture {
        fn new(tweak: impl FnOnce(&mut Config)) -> Fixture {
            let root = std::env::temp_dir().join(format!(
                "engine-jobs-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            let bin = root.join("bin");
            std::fs::create_dir_all(&bin).unwrap();
            std::os::unix::fs::symlink(
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/fake-ffprobe/ffprobe"
                ),
                bin.join("ffprobe"),
            )
            .unwrap();
            let mut cfg = Config {
                jobs_dir: root.join("jobs"),
                ffmpeg_dir: bin,
                ..Config::default()
            };
            tweak(&mut cfg);
            Fixture { root, cfg }
        }

        fn jobs(&self) -> Jobs {
            Jobs::new(Arc::new(self.cfg.clone()))
        }

        fn bin(&self) -> &Path {
            &self.cfg.ffmpeg_dir
        }

        fn release(&self) {
            std::fs::write(self.bin().join("release"), "").unwrap();
        }

        fn started(&self) -> Vec<String> {
            std::fs::read_to_string(self.bin().join("order.txt"))
                .unwrap_or_default()
                .lines()
                .map(ToOwned::to_owned)
                .collect()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn id(n: u8) -> String {
        format!("{n:032x}")
    }

    fn analyze_req(n: u8, path: &str, priority: Priority) -> JobRequest {
        JobRequest {
            job_id: id(n),
            priority,
            spec: JobSpec::Analyze { path: path.into() },
        }
    }

    async fn until(mut cond: impl FnMut() -> bool) {
        for _ in 0..200 {
            if cond() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("condition not reached in time");
    }

    async fn finished(jobs: &Jobs, n: u8) -> JobStatus {
        until(|| {
            jobs.status(&id(n))
                .is_ok_and(|s| matches!(s.state, JobState::Done | JobState::Failed))
        })
        .await;
        jobs.status(&id(n)).unwrap()
    }

    #[tokio::test]
    async fn interactive_jumps_queue() {
        let f = Fixture::new(|c| c.max_jobs = 1);
        let jobs = f.jobs();
        jobs.submit(analyze_req(1, "/m/blocker", Priority::Background))
            .unwrap();
        until(|| jobs.status(&id(1)).unwrap().state == JobState::Running).await;
        jobs.submit(analyze_req(2, "/m/b", Priority::Background))
            .unwrap();
        jobs.submit(analyze_req(3, "/m/i", Priority::Interactive))
            .unwrap();
        f.release();
        finished(&jobs, 2).await;
        finished(&jobs, 3).await;
        assert_eq!(f.started(), ["/m/blocker", "/m/i", "/m/b"]);
    }

    #[tokio::test]
    async fn queue_full_429() {
        let f = Fixture::new(|c| {
            c.max_jobs = 1;
            c.max_queued = 2;
        });
        let jobs = f.jobs();
        jobs.submit(analyze_req(1, "/m/blocker", Priority::Background))
            .unwrap();
        until(|| jobs.status(&id(1)).unwrap().state == JobState::Running).await;
        jobs.submit(analyze_req(2, "/m/a", Priority::Background))
            .unwrap();
        jobs.submit(analyze_req(3, "/m/b", Priority::Background))
            .unwrap();
        let err = jobs
            .submit(analyze_req(4, "/m/c", Priority::Background))
            .unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(err.code, "queue_full");
        assert_eq!(err.retry_after, Some(30));
        f.release();
    }

    #[tokio::test]
    async fn duplicate_409() {
        let f = Fixture::new(|_| {});
        let jobs = f.jobs();
        jobs.submit(analyze_req(1, "/m/a", Priority::Background))
            .unwrap();
        let err = jobs
            .submit(analyze_req(1, "/m/a", Priority::Background))
            .unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::CONFLICT);
        assert_eq!(err.code, "job_exists");
    }

    #[tokio::test]
    async fn finished_job_survives_restart() {
        let f = Fixture::new(|_| {});
        let jobs = f.jobs();
        jobs.submit(analyze_req(1, "/m/a", Priority::Background))
            .unwrap();
        let first = finished(&jobs, 1).await;
        assert_eq!(first.state, JobState::Done);
        assert!(first.result.is_some());

        let fresh = f.jobs();
        assert_eq!(fresh.status(&id(1)).unwrap(), first);
        let err = fresh
            .submit(analyze_req(1, "/m/a", Priority::Background))
            .unwrap_err();
        assert_eq!(err.code, "job_exists");
    }

    #[tokio::test]
    async fn failed_job_records_error() {
        let f = Fixture::new(|_| {});
        let jobs = f.jobs();
        jobs.submit(analyze_req(1, "/m/bad", Priority::Background))
            .unwrap();
        let s = finished(&jobs, 1).await;
        assert_eq!(s.state, JobState::Failed);
        assert!(s.error.unwrap().contains("no such file"));
        assert_eq!(f.jobs().status(&id(1)).unwrap().state, JobState::Failed);
    }

    #[tokio::test]
    async fn boot_sweep_drops_unfinished() {
        let f = Fixture::new(|_| {});
        let unfinished = f.cfg.jobs_dir.join(id(1));
        let finished = f.cfg.jobs_dir.join(id(2));
        std::fs::create_dir_all(&unfinished).unwrap();
        std::fs::create_dir_all(&finished).unwrap();
        std::fs::write(finished.join(DONE_FILE), r#"{"state":"done"}"#).unwrap();
        f.jobs().sweep_boot();
        assert!(!unfinished.exists());
        assert!(finished.exists());
    }

    #[tokio::test]
    async fn boot_sweep_spares_foreign_entries() {
        let f = Fixture::new(|_| {});
        let foreign = f.cfg.jobs_dir.join("photos");
        let upper = f.cfg.jobs_dir.join(id(10).to_uppercase());
        let ours = f.cfg.jobs_dir.join(id(10));
        for dir in [&foreign, &upper, &ours] {
            std::fs::create_dir_all(dir).unwrap();
        }
        f.jobs().sweep_boot();
        assert!(foreign.exists());
        assert!(upper.exists());
        assert!(!ours.exists());
    }

    #[tokio::test]
    async fn ttl_sweep() {
        let f = Fixture::new(|_| {});
        let old = f.cfg.jobs_dir.join(id(1));
        let fresh = f.cfg.jobs_dir.join(id(2));
        for dir in [&old, &fresh] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join(DONE_FILE), r#"{"state":"done"}"#).unwrap();
        }
        let eight_days = Duration::from_secs(8 * 24 * 3600);
        filetime::set_file_mtime(
            old.join(DONE_FILE),
            filetime::FileTime::from_system_time(SystemTime::now() - eight_days),
        )
        .unwrap();
        f.jobs().sweep_ttl_once();
        assert!(!old.exists());
        assert!(fresh.exists());
    }

    #[tokio::test]
    async fn cancel_running() {
        let f = Fixture::new(|_| {});
        let jobs = f.jobs();
        jobs.submit(analyze_req(1, "/m/blocker", Priority::Background))
            .unwrap();
        until(|| jobs.status(&id(1)).unwrap().state == JobState::Running).await;
        let dir = f.cfg.jobs_dir.join(id(1));
        assert!(dir.exists());
        jobs.cancel(&id(1)).unwrap();
        assert!(!dir.exists());
        assert_eq!(jobs.status(&id(1)).unwrap_err().code, "not_found");
        assert_eq!(jobs.cancel(&id(1)).unwrap_err().code, "not_found");
        f.release();
    }

    #[tokio::test]
    async fn cancel_queued_never_runs() {
        let f = Fixture::new(|c| c.max_jobs = 1);
        let jobs = f.jobs();
        jobs.submit(analyze_req(1, "/m/blocker", Priority::Background))
            .unwrap();
        until(|| jobs.status(&id(1)).unwrap().state == JobState::Running).await;
        jobs.submit(analyze_req(2, "/m/q", Priority::Background))
            .unwrap();
        jobs.cancel(&id(2)).unwrap();
        f.release();
        finished(&jobs, 1).await;
        assert_eq!(f.started(), ["/m/blocker"]);
    }

    #[tokio::test]
    async fn shutdown_aborts_running_and_drops_queue() {
        let f = Fixture::new(|c| c.max_jobs = 1);
        let jobs = f.jobs();
        jobs.submit(analyze_req(1, "/m/blocker", Priority::Background))
            .unwrap();
        until(|| jobs.status(&id(1)).unwrap().state == JobState::Running).await;
        jobs.submit(analyze_req(2, "/m/q", Priority::Background))
            .unwrap();
        jobs.shutdown();
        f.release();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(jobs.status(&id(1)).unwrap().state, JobState::Running);
        assert_eq!(f.started(), ["/m/blocker"]);
    }

    #[test]
    fn file_names_stay_inside() {
        assert!(name_ok("a.vtt"));
        assert!(name_ok("fonts/A.ttf"));
        for bad in [
            "../done.json",
            "/etc/passwd",
            "fonts\\..\\x",
            "",
            "a//b",
            "./a",
        ] {
            assert!(!name_ok(bad), "{bad:?}");
        }
    }
}
