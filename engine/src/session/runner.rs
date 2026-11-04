use std::collections::BTreeSet;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex as AsyncMutex, OwnedSemaphorePermit};
use tokio::time;

use super::{
    AudioOut, KEEP_BEHIND_SECS, RESUME_MARGIN_SECS, RUN_AHEAD_SECS, SEGMENT_WAIT, Served, VideoOut,
    hw_name, unix_now,
};
use crate::analysis::Analysis;
use crate::args::{RunSpec, live_args};
use crate::caps::Capabilities;
use crate::config::Config;
use crate::decision::{Mode, Plan, VideoAction};
use crate::error::ApiError;
use crate::hw::{Backend, Hw};

const POLL: Duration = Duration::from_millis(25);
const MONITOR_INTERVAL: Duration = Duration::from_millis(50);

pub(super) struct Live {
    pub mode: Mode,
    pub hw: &'static str,
    pub video: VideoOut,
    pub audio: AudioOut,
    pub bitrate: Option<u64>,
    pub speed: Option<Arc<AtomicU64>>,
}

impl Live {
    pub fn speed(&self) -> Option<f64> {
        let v = f64::from_bits(self.speed.as_ref()?.load(Ordering::Relaxed));
        (!v.is_nan()).then_some(v)
    }
}

fn no_speed() -> u64 {
    f64::NAN.to_bits()
}

#[derive(Debug, Clone, PartialEq)]
enum Status {
    Running,
    Stopped,
    Done,
    Failed(String),
}

pub(super) struct Run {
    id: u64,
    start: u32,
    edge: u32,
    child: Option<Child>,
    status: Status,
    reported: bool,
    stderr_tail: Arc<Mutex<String>>,
}

pub(super) struct State {
    pub closed: bool,
    pub next_run_id: u64,
    pub caps: Capabilities,
    pub analysis: Arc<Analysis>,
    pub plan: Plan,
    pub hw: Hw,
    pub fell_back: bool,
    pub bounds: Vec<f64>,
    pub master: String,
    pub media: String,
    pub completed: BTreeSet<u32>,
    pub fetched_t: f64,
    pub run: Option<Run>,
    pub permit: Option<OwnedSemaphorePermit>,
}

impl State {
    fn edge_time(&self) -> f64 {
        let edge = self.run.as_ref().map_or(0, |r| r.edge) as usize;
        self.bounds
            .get(edge)
            .copied()
            .unwrap_or(self.analysis.duration)
    }

    fn segment_end(&self, i: u32) -> f64 {
        self.bounds
            .get(i as usize + 1)
            .copied()
            .unwrap_or(self.analysis.duration)
    }
}

pub(super) struct Session {
    pub id: String,
    pub dir: PathBuf,
    pub input: PathBuf,
    pub cfg: Arc<Config>,
    pub epoch: AtomicU64,
    pub state: AsyncMutex<State>,
    pub live: Mutex<Live>,
    pub last_fetch: AtomicU64,
    pub bytes: AtomicU64,
    activity: Mutex<Instant>,
}

enum Need {
    Init,
    Segment(u32),
}

enum Next {
    Ready(PathBuf),
    Wait,
    Restart(u32),
    Fail(ApiError),
}

fn segment_index(name: &str) -> Option<u32> {
    name.strip_prefix("seg_")?
        .strip_suffix(".m4s")?
        .parse()
        .ok()
}

fn leading_segment_number(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("seg_")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn parse_speed(line: &str) -> Option<f64> {
    line.strip_prefix("speed=")?
        .trim()
        .strip_suffix('x')?
        .trim()
        .parse()
        .ok()
}

fn exit_message(tail: &str, status: &std::process::ExitStatus) -> String {
    let tail = tail.trim();
    if tail.is_empty() {
        format!("ffmpeg exited with {status}")
    } else {
        format!("ffmpeg exited with {status}: {tail}")
    }
}

impl Session {
    pub fn new(
        id: String,
        dir: PathBuf,
        input: PathBuf,
        cfg: Arc<Config>,
        state: State,
        live: Live,
    ) -> Self {
        Self {
            id,
            dir,
            input,
            cfg,
            epoch: AtomicU64::new(0),
            state: AsyncMutex::new(state),
            live: Mutex::new(live),
            last_fetch: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            activity: Mutex::new(Instant::now()),
        }
    }

    pub fn touch(&self) {
        *self.activity.lock().unwrap() = Instant::now();
    }

    pub fn idle(&self) -> Duration {
        self.activity.lock().unwrap().elapsed()
    }

    pub fn stamp_fetch(&self) {
        self.last_fetch.store(unix_now(), Ordering::Relaxed);
    }

    fn clear_speed(&self) {
        if let Some(s) = &self.live.lock().unwrap().speed {
            s.store(no_speed(), Ordering::Relaxed);
        }
    }

    pub async fn begin(self: &Arc<Self>, st: &mut State, start: u32) -> Result<(), ApiError> {
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| ApiError::ffmpeg_unavailable(format!("session dir: {e}")))?;
        for (name, body) in [("master.m3u8", &st.master), ("media.m3u8", &st.media)] {
            tokio::fs::write(self.dir.join(name), body)
                .await
                .map_err(|e| ApiError::ffmpeg_unavailable(format!("session dir: {e}")))?;
        }
        self.spawn_run(st, start).await
    }

    async fn refresh(&self, st: &mut State) {
        let Some(run) = st.run.as_mut() else { return };
        let path = self.dir.join(format!("ffmpeg_{}.m3u8", run.start));
        let Ok(text) = tokio::fs::read_to_string(&path).await else {
            return;
        };
        let known = run.edge;
        for line in text.lines() {
            if let Some(i) = segment_index(line.trim())
                && i >= known
            {
                st.completed.insert(i);
                run.edge = run.edge.max(i + 1);
            }
        }
    }

    pub async fn stop_run(&self, st: &mut State) {
        if let Some(run) = st.run.as_mut()
            && run.status == Status::Running
        {
            if let Some(mut child) = run.child.take() {
                let _ = child.kill().await;
            }
            run.status = Status::Stopped;
        }
        self.refresh(st).await;
        self.clear_speed();
    }

    async fn clear_from(&self, start: u32) {
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else {
            return;
        };
        let playlist = format!("ffmpeg_{start}.m3u8");
        let init = format!("init_{start}.mp4");
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stale = name == playlist
                || name == init
                || leading_segment_number(&name).is_some_and(|i| i >= start);
            if stale {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }

    fn publish(&self, st: &State) {
        let mut live = self.live.lock().unwrap();
        live.hw = hw_name(&st.plan, &st.hw);
    }

    // Boxed: spawn_run starts a monitor that calls tick, which can call spawn_run
    // again, and the cycle defeats the compiler's Send inference.
    fn spawn_run<'a>(
        self: &'a Arc<Self>,
        st: &'a mut State,
        start: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), ApiError>> + Send + 'a>> {
        Box::pin(self.spawn_run_inner(st, start))
    }

    async fn spawn_run_inner(self: &Arc<Self>, st: &mut State, start: u32) -> Result<(), ApiError> {
        self.stop_run(st).await;
        self.clear_from(start).await;
        st.completed.retain(|i| *i < start);

        let spec = RunSpec {
            input: &self.input,
            plan: &st.plan,
            analysis: &st.analysis,
            hw: &st.hw,
            start_index: start,
            start_time: st.bounds[start as usize],
            out_dir: &self.dir,
        };
        let mut child = Command::new(self.cfg.ffmpeg())
            .args(live_args(&spec))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ApiError::ffmpeg_unavailable(format!("cannot start ffmpeg: {e}")))?;

        let speed = Arc::new(AtomicU64::new(no_speed()));
        if let Some(out) = child.stdout.take() {
            let speed = speed.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(out).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(v) = parse_speed(&line) {
                        speed.store(v.to_bits(), Ordering::Relaxed);
                    }
                }
            });
        }
        let stderr_tail = Arc::new(Mutex::new(String::new()));
        if let Some(err) = child.stderr.take() {
            let tail = stderr_tail.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    *tail.lock().unwrap() = line;
                }
            });
        }

        let id = st.next_run_id;
        st.next_run_id += 1;
        st.run = Some(Run {
            id,
            start,
            edge: start,
            child: Some(child),
            status: Status::Running,
            reported: false,
            stderr_tail,
        });
        self.live.lock().unwrap().speed = Some(speed);
        self.publish(st);

        let this = self.clone();
        tokio::spawn(async move {
            loop {
                time::sleep(MONITOR_INTERVAL).await;
                let mut st = this.state.lock().await;
                let current = st.run.as_ref().is_some_and(|r| r.id == id);
                if st.closed || !current {
                    return;
                }
                this.tick(&mut st).await;
                if st
                    .run
                    .as_ref()
                    .is_none_or(|r| r.id != id || r.status != Status::Running)
                {
                    return;
                }
            }
        });
        Ok(())
    }

    async fn tick(self: &Arc<Self>, st: &mut State) {
        let exit = match st.run.as_mut() {
            Some(run) if run.status == Status::Running => {
                run.child.as_mut().and_then(|c| c.try_wait().ok().flatten())
            }
            _ => return,
        };
        self.refresh(st).await;
        let Some(status) = exit else {
            if st.edge_time() - st.fetched_t > RUN_AHEAD_SECS {
                self.stop_run(st).await;
            }
            return;
        };
        self.clear_speed();
        let transcoding = matches!(st.plan.video, VideoAction::Transcode { .. });
        let Some(run) = st.run.as_mut() else { return };
        run.child = None;
        if status.success() {
            run.status = Status::Done;
            return;
        }
        let start = run.start;
        let no_output = run.edge == run.start;
        let message = exit_message(&run.stderr_tail.lock().unwrap(), &status);
        if no_output && transcoding && st.hw.backend != Backend::Cpu && !st.fell_back {
            st.fell_back = true;
            st.hw = Hw::new(Backend::Cpu, String::new(), st.hw.encoders, false);
            if let Err(e) = self.spawn_run(st, start).await
                && let Some(run) = st.run.as_mut()
            {
                run.status = Status::Failed(e.message);
            }
            return;
        }
        run.status = Status::Failed(message);
    }

    fn decide_next(&self, st: &mut State, need: &Need) -> Next {
        let restart_at = match need {
            Need::Init => st.run.as_ref().map_or(0, |r| r.start),
            Need::Segment(n) => *n,
        };
        match need {
            Need::Segment(n) if st.completed.contains(n) => {
                return Next::Ready(self.dir.join(format!("seg_{n}.m4s")));
            }
            Need::Init => {
                if let Some(run) = &st.run
                    && run.edge > run.start
                {
                    return Next::Ready(self.dir.join(format!("init_{}.mp4", run.start)));
                }
            }
            Need::Segment(_) => {}
        }
        let edge_time = st.edge_time();
        let bounds = &st.bounds;
        let Some(run) = st.run.as_mut() else {
            return Next::Restart(restart_at);
        };
        if let Status::Failed(msg) = &run.status
            && !run.reported
        {
            run.reported = true;
            return Next::Fail(ApiError::ffmpeg_failed(msg.clone()));
        }
        let reachable = match need {
            Need::Init => true,
            Need::Segment(n) => {
                *n >= run.edge && bounds[*n as usize] < edge_time + RESUME_MARGIN_SECS
            }
        };
        if run.status == Status::Running && reachable {
            Next::Wait
        } else {
            Next::Restart(restart_at)
        }
    }

    pub async fn serve(self: &Arc<Self>, file: &str) -> Result<Served, ApiError> {
        let need = match file {
            "master.m3u8" | "media.m3u8" => {
                let st = self.state.lock().await;
                if st.closed || st.plan.mode == Mode::Direct {
                    return Err(ApiError::not_found("unknown session"));
                }
                let body = if file == "master.m3u8" {
                    st.master.clone()
                } else {
                    st.media.clone()
                };
                return Ok(Served::Text(body));
            }
            "init.mp4" => Need::Init,
            other => match segment_index(other) {
                Some(n) => Need::Segment(n),
                None => return Err(ApiError::not_found("no such file")),
            },
        };

        let epoch = self.epoch.load(Ordering::SeqCst);
        let deadline = time::Instant::now() + SEGMENT_WAIT;
        loop {
            {
                let mut st = self.state.lock().await;
                if st.closed || st.plan.mode == Mode::Direct {
                    return Err(ApiError::not_found("unknown session"));
                }
                if self.epoch.load(Ordering::SeqCst) != epoch {
                    return Err(ApiError::session_restarted());
                }
                if let Need::Segment(n) = need
                    && n as usize >= st.bounds.len()
                {
                    return Err(ApiError::not_found("segment is past the end"));
                }
                self.tick(&mut st).await;
                match self.decide_next(&mut st, &need) {
                    Next::Ready(path) => {
                        if let Need::Segment(n) = need {
                            self.after_fetch(&mut st, n).await;
                        }
                        return Ok(Served::File(path));
                    }
                    Next::Fail(e) => return Err(e),
                    Next::Restart(at) => {
                        if matches!(need, Need::Segment(_)) {
                            st.fetched_t = st.bounds[at as usize];
                        }
                        self.spawn_run(&mut st, at).await?
                    }
                    Next::Wait => {}
                }
            }
            if time::Instant::now() >= deadline {
                return Err(ApiError::ffmpeg_failed("timed out waiting for ffmpeg"));
            }
            time::sleep(POLL).await;
        }
    }

    async fn after_fetch(self: &Arc<Self>, st: &mut State, n: u32) {
        st.fetched_t = st.bounds[n as usize];
        self.window(st).await;
        self.evict(st).await;
    }

    async fn window(self: &Arc<Self>, st: &mut State) {
        let Some(run) = &st.run else { return };
        let ahead = st.edge_time() - st.fetched_t;
        match run.status {
            Status::Running if ahead > RUN_AHEAD_SECS => self.stop_run(st).await,
            Status::Stopped | Status::Done
                if (run.edge as usize) < st.bounds.len() && ahead < RESUME_MARGIN_SECS =>
            {
                let edge = run.edge;
                let _ = self.spawn_run(st, edge).await;
            }
            _ => {}
        }
    }

    async fn evict(&self, st: &mut State) {
        let cutoff = st.fetched_t - KEEP_BEHIND_SECS;
        let old: Vec<u32> = st
            .completed
            .iter()
            .copied()
            .take_while(|i| st.segment_end(*i) < cutoff)
            .collect();
        for i in old {
            st.completed.remove(&i);
            let _ = tokio::fs::remove_file(self.dir.join(format!("seg_{i}.m4s"))).await;
        }
    }
}
