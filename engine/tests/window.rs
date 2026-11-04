#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use engine::analysis::{Analysis, AudioStream, VideoStream};
use engine::caps::{AudioCaps, Capabilities, HdrCaps, VideoCap};
use engine::config::Config;
use engine::decision::{EncoderSet, Mode};
use engine::hw::{Backend, Hw};
use engine::session::{PatchRequest, SEGMENT_WAIT, Served, SessionInfo, Sessions, StartRequest};

const FAKE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/fake-ffmpeg/ffmpeg"
);

const ALL: EncoderSet = EncoderSet {
    h264: true,
    hevc: true,
    av1: true,
};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Env {
    root: PathBuf,
    sessions: Sessions,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Env {
    fn session_dir(&self, id: &str) -> PathBuf {
        self.root.join("live").join(id)
    }

    fn argv(&self, id: &str, start: u32) -> Vec<String> {
        std::fs::read_to_string(self.session_dir(id).join(format!("argv_{start}.txt")))
            .map(|t| t.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    fn playlist_len(&self, id: &str, start: u32) -> usize {
        std::fs::read_to_string(self.session_dir(id).join(format!("ffmpeg_{start}.m3u8")))
            .map(|t| t.lines().filter(|l| l.ends_with(".m4s")).count())
            .unwrap_or(0)
    }

    fn seg_files(&self, id: &str) -> usize {
        std::fs::read_dir(self.session_dir(id))
            .map(|d| {
                d.flatten()
                    .filter(|e| e.file_name().to_string_lossy().starts_with("seg_"))
                    .count()
            })
            .unwrap_or(0)
    }

    fn info(&self) -> SessionInfo {
        self.sessions.list().remove(0)
    }

    async fn fetch(&self, id: &str, n: u32) -> Result<Served, engine::error::ApiError> {
        self.sessions.fetch_hls(id, &format!("seg_{n}.m4s")).await
    }
}

struct Setup {
    fake_env: &'static str,
    max_transcodes: usize,
    orphan_timeout: Duration,
    reap: Duration,
    hw: Hw,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            fake_env: "",
            max_transcodes: 4,
            orphan_timeout: Duration::from_secs(3600),
            reap: Duration::from_secs(60),
            hw: Hw::new(Backend::Cpu, String::new(), ALL, false),
        }
    }
}

fn env(s: Setup) -> Env {
    let root = std::env::temp_dir().join(format!(
        "engine-window-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::os::unix::fs::symlink(FAKE, bin.join("ffmpeg")).unwrap();
    std::fs::write(bin.join("fake.env"), s.fake_env).unwrap();
    std::fs::write(root.join("input.mkv"), b"x").unwrap();
    let cfg = Config {
        tmp_dir: root.join("live"),
        ffmpeg_dir: bin,
        max_transcodes: s.max_transcodes,
        orphan_timeout: s.orphan_timeout,
        ..Config::default()
    };
    let sessions = Sessions::with_reap_interval(Arc::new(cfg), Arc::new(s.hw), s.reap);
    Env { root, sessions }
}

fn id(n: u8) -> String {
    format!("{n:032x}")
}

fn analysis() -> Analysis {
    Analysis {
        version: 1,
        container: "mkv".into(),
        duration: 200.0,
        bit_rate: 8_000_000,
        video: vec![VideoStream {
            index: 0,
            codec: "hevc".into(),
            profile: "Main".into(),
            level: Some(120),
            bit_depth: 8,
            pix_fmt: "yuv420p".into(),
            width: 1920,
            height: 1080,
            sar: (1, 1),
            fps: 24.0,
            field_order: "progressive".into(),
            ..Default::default()
        }],
        audio: vec![
            AudioStream {
                index: 1,
                codec: "aac".into(),
                channels: 2,
                layout: "stereo".into(),
                default: true,
                ..Default::default()
            },
            AudioStream {
                index: 2,
                codec: "aac".into(),
                channels: 2,
                layout: "stereo".into(),
                ..Default::default()
            },
        ],
        keyframes: (0..200).map(f64::from).collect(),
        ..Default::default()
    }
}

fn caps(codec: &str) -> Capabilities {
    Capabilities {
        video: vec![VideoCap {
            codec: codec.into(),
            profiles: vec![],
            max_level: None,
            max_width: 3840,
            max_height: 2160,
            max_fps: 60.0,
        }],
        hdr: HdrCaps {
            formats: vec![],
            dv_profiles: vec![],
        },
        audio: AudioCaps {
            codecs: vec!["aac".into()],
            max_channels: 6,
            passthrough: vec![],
        },
        containers: vec!["mp4".into()],
        subtitles: vec![],
        hls_keyframe_exact: true,
    }
}

fn remux_req(e: &Env, id: &str) -> StartRequest {
    request(e, id, "hevc")
}

fn transcode_req(e: &Env, id: &str) -> StartRequest {
    request(e, id, "h264")
}

fn request(e: &Env, id: &str, codec: &str) -> StartRequest {
    StartRequest {
        session_id: id.to_owned(),
        path: e.root.join("input.mkv").display().to_string(),
        capabilities: caps(codec),
        analysis: Some(analysis()),
        position: 0.0,
        audio_index: None,
        burn_in_subtitle: None,
        max_bitrate: None,
    }
}

async fn until(what: &str, secs: u64, mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !f() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn file(s: Served) -> PathBuf {
    match s {
        Served::File(p) => p,
        Served::Text(_) => panic!("expected a file"),
    }
}

#[tokio::test]
async fn start_serves_first_segment() {
    let e = env(Setup::default());
    let sid = id(1);
    let r = e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    assert_eq!(r.mode, Mode::Remux);
    assert_eq!(r.url_path, "master.m3u8");
    let path = file(e.fetch(&sid, 0).await.unwrap());
    assert!(path.exists());
    assert!(Path::new(&path).ends_with("seg_0.m4s"));
    let Served::Text(master) = e.sessions.fetch_hls(&sid, "master.m3u8").await.unwrap() else {
        panic!("expected text");
    };
    assert!(master.contains("media.m3u8"));
    assert!(file(e.sessions.fetch_hls(&sid, "init.mp4").await.unwrap()).exists());
}

#[tokio::test]
async fn seek_outside_range_restarts() {
    let e = env(Setup::default());
    let sid = id(2);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    let path = file(e.fetch(&sid, 150).await.unwrap());
    assert!(path.exists());
    let argv = e.argv(&sid, 150);
    assert_eq!(argv.len(), 1);
    assert!(argv[0].contains("-start_number 150"));
}

#[tokio::test]
async fn far_seek_survives_run_ahead() {
    let e = env(Setup {
        fake_env: "FAKE_SEG_MS=500",
        ..Setup::default()
    });
    let sid = id(7);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    e.fetch(&sid, 0).await.unwrap();
    assert!(file(e.fetch(&sid, 190).await.unwrap()).exists());
    assert_eq!(e.argv(&sid, 190).len(), 1);
}

#[tokio::test]
async fn run_ahead_stops_ffmpeg() {
    let e = env(Setup {
        fake_env: "FAKE_SEGMENTS=400\nFAKE_SEG_MS=10",
        ..Setup::default()
    });
    let sid = id(3);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    e.fetch(&sid, 0).await.unwrap();
    until("the edge to pass the window", 20, || {
        e.playlist_len(&sid, 0) > 180
    })
    .await;
    until("ffmpeg to stop", 5, || e.info().speed.is_none()).await;
    let before = e.seg_files(&sid);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(e.seg_files(&sid), before);
    assert!(e.info().speed.is_none());
}

#[tokio::test]
async fn resume_near_edge() {
    let e = env(Setup {
        fake_env: "FAKE_SEGMENTS=400\nFAKE_SEG_MS=10",
        ..Setup::default()
    });
    let sid = id(4);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    e.fetch(&sid, 0).await.unwrap();
    until("the edge to pass the window", 20, || {
        e.playlist_len(&sid, 0) > 180
    })
    .await;
    until("ffmpeg to stop", 5, || e.info().speed.is_none()).await;
    let edge = e.playlist_len(&sid, 0) as u32;

    for n in 1..edge - 59 {
        e.fetch(&sid, n).await.unwrap();
        assert!(e.argv(&sid, edge).is_empty(), "resumed early at {n}");
    }
    e.fetch(&sid, edge - 59).await.unwrap();
    until("the resumed run", 5, || !e.argv(&sid, edge).is_empty()).await;
}

#[tokio::test]
async fn evicts_behind() {
    let e = env(Setup {
        fake_env: "FAKE_SEG_MS=10",
        ..Setup::default()
    });
    let sid = id(5);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    let first = file(e.fetch(&sid, 0).await.unwrap());
    assert!(first.exists());
    e.fetch(&sid, 190).await.unwrap();
    assert!(!first.exists());
}

#[tokio::test]
async fn seek_back_to_evicted_restarts() {
    let e = env(Setup {
        fake_env: "FAKE_SEGMENTS=400\nFAKE_SEG_MS=20",
        ..Setup::default()
    });
    let sid = id(15);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    for n in (0..=190).step_by(19) {
        e.fetch(&sid, n).await.unwrap();
    }
    assert!(e.argv(&sid, 5).is_empty());
    let path = tokio::time::timeout(Duration::from_secs(10), e.fetch(&sid, 5))
        .await
        .expect("seek back hung waiting for an evicted segment")
        .unwrap();
    assert!(file(path).exists());
    assert_eq!(e.argv(&sid, 5).len(), 1);
}

#[tokio::test]
async fn paused_session_survives() {
    let e = env(Setup {
        reap: Duration::from_secs(1),
        ..Setup::default()
    });
    e.sessions.start(remux_req(&e, &id(6))).await.unwrap();
    let reaper = e.sessions.spawn_reaper();
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(e.sessions.list().len(), 1);
    reaper.abort();
}

#[tokio::test]
async fn reaper_deletes_orphans() {
    let e = env(Setup {
        orphan_timeout: Duration::from_secs(1),
        reap: Duration::from_secs(1),
        ..Setup::default()
    });
    let sid = id(7);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    let reaper = e.sessions.spawn_reaper();
    until("the reaper", 6, || e.sessions.list().is_empty()).await;
    assert!(!e.session_dir(&sid).exists());
    reaper.abort();
}

#[tokio::test]
async fn transcode_limit() {
    let e = env(Setup {
        max_transcodes: 1,
        ..Setup::default()
    });
    let first = e.sessions.start(transcode_req(&e, &id(8))).await.unwrap();
    assert_eq!(first.mode, Mode::Transcode);
    let err = e
        .sessions
        .start(transcode_req(&e, &id(9)))
        .await
        .unwrap_err();
    assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(err.code, "transcode_limit");
    assert_eq!(err.retry_after, Some(10));
    let remux = e.sessions.start(remux_req(&e, &id(10))).await.unwrap();
    assert_eq!(remux.mode, Mode::Remux);

    e.sessions.delete(&id(8)).await.unwrap();
    e.sessions.start(transcode_req(&e, &id(9))).await.unwrap();
}

#[tokio::test]
async fn concurrent_fetch_single_restart() {
    let e = env(Setup::default());
    let sid = id(11);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    let (a, b) = tokio::join!(e.fetch(&sid, 120), e.fetch(&sid, 120));
    assert!(file(a.unwrap()).exists());
    assert!(file(b.unwrap()).exists());
    assert_eq!(e.argv(&sid, 120).len(), 1);
}

#[tokio::test]
async fn patch_never_serves_old_segments() {
    let e = env(Setup::default());
    let sid = id(12);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    let sessions = e.sessions.clone();
    let waiting_sid = sid.clone();
    let waiter = tokio::spawn(async move { sessions.fetch_hls(&waiting_sid, "seg_50.m4s").await });
    tokio::time::sleep(Duration::from_millis(300)).await;

    e.sessions
        .patch(
            &sid,
            PatchRequest {
                audio_index: Some(2),
                burn_in_subtitle: None,
                max_bitrate: None,
                position: 50.0,
            },
        )
        .await
        .unwrap();
    let err = waiter.await.unwrap().unwrap_err();
    assert_eq!(err.code, "session_restarted");

    let path = file(e.fetch(&sid, 50).await.unwrap());
    assert!(path.exists());
    let argv = e.argv(&sid, 50);
    assert_eq!(argv.len(), 1);
    assert!(argv[0].contains("-map 0:2"));
}

#[tokio::test]
async fn ffmpeg_dies_mid_run() {
    let e = env(Setup {
        fake_env: "FAKE_FAIL_AFTER=1",
        ..Setup::default()
    });
    let sid = id(13);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    let began = Instant::now();
    let err = e.fetch(&sid, 5).await.unwrap_err();
    assert_eq!(err.code, "ffmpeg_failed");
    assert!(began.elapsed() < SEGMENT_WAIT);

    assert!(e.argv(&sid, 5).is_empty());
    let _ = e.fetch(&sid, 5).await;
    assert_eq!(e.argv(&sid, 5).len(), 1);
}

#[tokio::test]
async fn past_end_is_404() {
    let e = env(Setup::default());
    let sid = id(14);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    let err = e.fetch(&sid, 9999).await.unwrap_err();
    assert_eq!(err.status, StatusCode::NOT_FOUND);
    until("the first run", 5, || e.argv(&sid, 0).len() == 1).await;
    assert!(e.argv(&sid, 9999).is_empty());
}

#[tokio::test]
async fn gpu_died_falls_back_to_cpu() {
    let e = env(Setup {
        fake_env: "FAKE_FAIL_FIRST=1",
        hw: Hw::new(Backend::Vaapi, String::new(), ALL, false),
        ..Setup::default()
    });
    let sid = id(15);
    let r = e.sessions.start(transcode_req(&e, &sid)).await.unwrap();
    assert_eq!(r.hw, "vaapi");
    until("the retry", 5, || e.argv(&sid, 0).len() == 2).await;
    let argv = e.argv(&sid, 0);
    assert!(argv[0].contains("-hwaccel"));
    assert!(!argv[1].contains("-hwaccel"));
    assert_eq!(e.info().hw, "cpu");
}
