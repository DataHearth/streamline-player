#![cfg(unix)]

use std::future::Future;
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
    hevc_first: false,
    dovi_tool: true,
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

    fn ffmpeg_alive(&self, id: &str) -> bool {
        let dir = self.session_dir(id).display().to_string();
        std::fs::read_dir("/proc")
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|p| std::fs::read(p.path().join("cmdline")).ok())
            .any(|cmd| String::from_utf8_lossy(&cmd).contains(&dir))
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
        version: engine::analysis::ANALYSIS_VERSION,
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

fn file_text(e: &Env, id: &str, name: &str) -> String {
    std::fs::read_to_string(e.session_dir(id).join(name)).unwrap()
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
    until("ffmpeg to stop", 5, || {
        e.info().speed.is_none() && !e.ffmpeg_alive(&sid)
    })
    .await;
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
    until("the reaper", 6, || {
        e.sessions.list().is_empty() && !e.session_dir(&sid).exists()
    })
    .await;
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
                audio_index: Some(Some(2)),
                position: Some(50.0),
                ..Default::default()
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

fn hw_codecs(codecs: &[&str]) -> Capabilities {
    let mut c = caps(codecs[0]);
    c.video = codecs.iter().map(|k| caps(k).video.remove(0)).collect();
    c
}

fn av1_gpu() -> Hw {
    let mut hw = Hw::new(
        Backend::Vaapi,
        String::new(),
        EncoderSet {
            h264: true,
            hevc: false,
            av1: true,
            hevc_first: false,
            dovi_tool: true,
        },
        false,
    );
    hw.cpu_encoders = EncoderSet {
        h264: true,
        hevc: false,
        av1: false,
        hevc_first: false,
        dovi_tool: true,
    };
    hw
}

#[tokio::test]
async fn gpu_decode_failure_keeps_gpu_encoder() {
    let e = env(Setup {
        fake_env: "FAKE_FAIL_RUNS=1",
        hw: av1_gpu(),
        ..Setup::default()
    });
    let sid = id(15);
    let mut req = transcode_req(&e, &sid);
    req.capabilities = hw_codecs(&["av1", "h264"]);
    let r = e.sessions.start(req).await.unwrap();
    assert_eq!(r.hw, "vaapi");
    until("the retry", 5, || e.argv(&sid, 0).len() == 2).await;
    let argv = e.argv(&sid, 0);
    assert!(argv[0].contains("-hwaccel"));
    assert!(!argv[1].contains("-hwaccel"));
    assert!(argv[1].contains("av1_vaapi"));
    assert_eq!(e.info().hw, "vaapi");
}

#[tokio::test]
async fn gpu_encode_failure_replans_for_cpu_encoders() {
    let e = env(Setup {
        fake_env: "FAKE_FAIL_RUNS=2",
        hw: av1_gpu(),
        ..Setup::default()
    });
    let sid = id(16);
    let mut req = transcode_req(&e, &sid);
    req.capabilities = hw_codecs(&["av1", "h264"]);
    e.sessions.start(req).await.unwrap();
    until("the cpu run", 5, || e.argv(&sid, 0).len() == 3).await;
    let argv = e.argv(&sid, 0);
    assert!(argv[1].contains("av1_vaapi"));
    assert!(argv[2].contains("libx264"));
    assert!(!argv[2].contains("libsvtav1"));
    assert_eq!(e.info().hw, "cpu");
    let master = file_text(&e, &sid, "master.m3u8");
    assert!(master.contains("avc1"), "{master}");
}

#[tokio::test]
async fn cpu_fallback_keeps_the_advertised_codec() {
    let mut hw = Hw::new(
        Backend::Vaapi,
        String::new(),
        EncoderSet {
            h264: true,
            hevc: false,
            av1: false,
            hevc_first: false,
            dovi_tool: true,
        },
        false,
    );
    hw.cpu_encoders = EncoderSet {
        h264: true,
        hevc: false,
        av1: true,
        hevc_first: false,
        dovi_tool: true,
    };
    let e = env(Setup {
        fake_env: "FAKE_FAIL_RUNS=2",
        hw,
        ..Setup::default()
    });
    let sid = id(18);
    let mut req = transcode_req(&e, &sid);
    req.capabilities = hw_codecs(&["av1", "h264"]);
    e.sessions.start(req).await.unwrap();
    until("the cpu run", 5, || e.argv(&sid, 0).len() == 3).await;
    let argv = e.argv(&sid, 0);
    assert!(argv[1].contains("h264_vaapi"));
    assert!(argv[2].contains("libx264"), "{}", argv[2]);
    let master = file_text(&e, &sid, "master.m3u8");
    assert!(master.contains("avc1"), "{master}");
}

#[tokio::test]
async fn failure_nobody_waited_on_restarts_silently() {
    let e = env(Setup {
        fake_env: "FAKE_FAIL_RUNS=1",
        ..Setup::default()
    });
    let sid = id(17);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    until("the first run", 5, || e.argv(&sid, 0).len() == 1).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    file(e.fetch(&sid, 0).await.unwrap());
    assert_eq!(e.argv(&sid, 0).len(), 2);
}

#[tokio::test]
async fn dropped_stop_leaves_run_stopped() {
    let e = env(Setup {
        fake_env: "FAKE_SEGMENTS=400\nFAKE_SEG_MS=999",
        ..Setup::default()
    });
    let sid = id(20);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    e.fetch(&sid, 0).await.unwrap();

    let mut patch = Box::pin(e.sessions.patch(
        &sid,
        PatchRequest {
            position: Some(0.0),
            ..Default::default()
        },
    ));
    while e.ffmpeg_alive(&sid) {
        let polled = std::future::poll_fn(|cx| std::task::Poll::Ready(patch.as_mut().poll(cx)));
        assert!(polled.await.is_pending(), "the patch finished unprompted");
        tokio::task::yield_now().await;
    }
    drop(patch);
    until("ffmpeg to die", 5, || !e.ffmpeg_alive(&sid)).await;

    let next = tokio::time::timeout(Duration::from_secs(10), e.fetch(&sid, 5))
        .await
        .expect("fetch waited on a run nobody owns");
    assert!(file(next.unwrap()).exists());
}

#[tokio::test]
async fn dropped_delete_still_tears_down() {
    let e = env(Setup {
        max_transcodes: 1,
        ..Setup::default()
    });
    let sid = id(21);
    e.sessions.start(transcode_req(&e, &sid)).await.unwrap();
    e.fetch(&sid, 0).await.unwrap();

    let mut delete = Box::pin(e.sessions.delete(&sid));
    let _ = std::future::poll_fn(|cx| std::task::Poll::Ready(delete.as_mut().poll(cx))).await;
    drop(delete);
    until("the teardown", 5, || {
        !e.ffmpeg_alive(&sid) && !e.session_dir(&sid).exists()
    })
    .await;
    assert!(e.sessions.list().is_empty());
    e.sessions
        .start(transcode_req(&e, &id(22)))
        .await
        .expect("the transcode permit leaked");
}

#[tokio::test]
async fn delete_wakes_waiting_fetch() {
    let e = env(Setup {
        fake_env: "FAKE_SEGMENTS=400\nFAKE_SEG_MS=999",
        ..Setup::default()
    });
    let sid = id(23);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    let sessions = e.sessions.clone();
    let waiting = sid.clone();
    let waiter = tokio::spawn(async move { sessions.fetch_hls(&waiting, "seg_30.m4s").await });
    until("the fetch to wait", 5, || e.ffmpeg_alive(&sid)).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    e.sessions.delete(&sid).await.unwrap();
    let err = tokio::time::timeout(Duration::from_secs(5), waiter)
        .await
        .expect("the waiting fetch outlived the delete")
        .unwrap()
        .unwrap_err();
    assert_eq!(err.status, StatusCode::NOT_FOUND);
    assert!(!e.ffmpeg_alive(&sid));
}

#[tokio::test]
async fn patch_keeps_absent_fields() {
    let e = env(Setup {
        fake_env: "FAKE_SEG_MS=10",
        ..Setup::default()
    });
    let sid = id(24);
    let mut req = remux_req(&e, &sid);
    req.audio_index = Some(2);
    e.sessions.start(req).await.unwrap();
    assert!(e.argv(&sid, 0).is_empty() || e.argv(&sid, 0)[0].contains("-map 0:2"));

    e.sessions
        .patch(
            &sid,
            PatchRequest {
                position: Some(50.0),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    until("the run at 50", 5, || e.argv(&sid, 50).len() == 1).await;
    assert!(e.argv(&sid, 50)[0].contains("-map 0:2"));

    e.sessions
        .patch(
            &sid,
            PatchRequest {
                audio_index: Some(None),
                position: Some(60.0),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    until("the run at 60", 5, || e.argv(&sid, 60).len() == 1).await;
    assert!(e.argv(&sid, 60)[0].contains("-map 0:1"));
}

#[tokio::test]
async fn patch_without_position_keeps_current() {
    let e = env(Setup {
        fake_env: "FAKE_SEG_MS=10",
        ..Setup::default()
    });
    let sid = id(25);
    e.sessions.start(remux_req(&e, &sid)).await.unwrap();
    e.fetch(&sid, 70).await.unwrap();

    e.sessions
        .patch(
            &sid,
            PatchRequest {
                audio_index: Some(Some(2)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    until("the run at 70", 5, || e.argv(&sid, 70).len() == 1).await;
    assert!(e.argv(&sid, 70)[0].contains("-map 0:2"));
    assert!(e.argv(&sid, 0).is_empty());
}

#[tokio::test]
async fn patch_keeps_and_clears_max_bitrate() {
    let e = env(Setup::default());
    let sid = id(26);
    let mut req = transcode_req(&e, &sid);
    req.max_bitrate = Some(1_000_000);
    e.sessions.start(req).await.unwrap();
    let capped = e.info().bitrate.unwrap();

    e.sessions
        .patch(
            &sid,
            PatchRequest {
                position: Some(10.0),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(e.info().bitrate, Some(capped));

    e.sessions
        .patch(
            &sid,
            PatchRequest {
                max_bitrate: Some(None),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(e.info().bitrate.unwrap() > capped);
}

#[tokio::test]
async fn boot_sweep_spares_foreign_entries() {
    let e = env(Setup::default());
    let live = e.root.join("live");
    let ours = live.join(id(30));
    let foreign_dir = live.join("somebody-elses");
    let foreign_file = live.join("notes.txt");
    let upper = live.join(id(30).to_uppercase());
    for d in [&ours, &foreign_dir, &upper] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(&foreign_file, b"x").unwrap();
    e.sessions.sweep_tmp();
    assert!(!ours.exists());
    assert!(foreign_dir.exists());
    assert!(foreign_file.exists());
    assert!(upper.exists());
}
