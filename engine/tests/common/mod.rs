#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use engine::analysis::{Analysis, AudioStream, VideoStream};
use engine::caps::{AudioCaps, Capabilities, HdrCaps, VideoCap};
use engine::config::Config;
use engine::decision::EncoderSet;
use engine::hw::{Backend, Hw};
use engine::jobs::Jobs;
use engine::session::Sessions;
use engine::token::{Scope, sign};
use engine::{AppState, app};
use serde_json::{Value, json};
use tower::ServiceExt;

pub const SECRET: &str = "test-secret";

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct Env {
    pub root: PathBuf,
    pub router: Router,
    pub sessions: Sessions,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub fn env() -> Env {
    let root = std::env::temp_dir().join(format!(
        "engine-http-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/fake-ffmpeg/ffmpeg"
            ),
            bin.join("ffmpeg"),
        )
        .unwrap();
        std::os::unix::fs::symlink(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/fake-ffprobe/ffprobe"
            ),
            bin.join("ffprobe"),
        )
        .unwrap();
        std::fs::write(bin.join("fake.env"), "").unwrap();
    }
    let cfg = Arc::new(Config {
        secret: Some(SECRET.into()),
        tmp_dir: root.join("live"),
        jobs_dir: root.join("jobs"),
        ffmpeg_dir: bin,
        ..Config::default()
    });
    let hw = Arc::new(Hw::new(
        Backend::Cpu,
        String::new(),
        EncoderSet {
            h264: true,
            hevc: true,
            av1: true,
        },
        false,
    ));
    let sessions = Sessions::new(cfg.clone(), hw.clone());
    let jobs = Jobs::new(cfg.clone());
    let router = app(AppState {
        cfg,
        secret: Arc::new(SECRET.as_bytes().to_vec()),
        hw,
        ffmpeg_version: Some("test".into()),
        sessions: sessions.clone(),
        jobs,
    });
    Env {
        root,
        router,
        sessions,
    }
}

pub fn id(n: u8) -> String {
    format!("{n:032x}")
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn token_query(id: &str, exp: u64) -> String {
    format!(
        "exp={exp}&sig={}",
        sign(SECRET.as_bytes(), Scope::Hls, id, exp)
    )
}

pub fn valid_query(id: &str) -> String {
    token_query(id, now() + 3600)
}

impl Env {
    pub async fn send(&self, req: Request<Body>) -> Response<Body> {
        self.router.clone().oneshot(req).await.unwrap()
    }

    pub async fn get(&self, uri: &str) -> Response<Body> {
        self.send(Request::get(uri).body(Body::empty()).unwrap())
            .await
    }

    pub async fn get_authed(&self, uri: &str) -> Response<Body> {
        self.send(
            Request::get(uri)
                .header("authorization", format!("Bearer {SECRET}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    pub async fn api(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut b = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {SECRET}"));
        let body = match body {
            Some(v) => {
                b = b.header("content-type", "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let res = self.send(b.body(body).unwrap()).await;
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let v = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, v)
    }
}

pub async fn body_bytes(res: Response<Body>) -> Vec<u8> {
    axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

pub fn analysis() -> Analysis {
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
        audio: vec![AudioStream {
            index: 1,
            codec: "aac".into(),
            channels: 2,
            layout: "stereo".into(),
            default: true,
            ..Default::default()
        }],
        keyframes: (0..200).map(f64::from).collect(),
        ..Default::default()
    }
}

pub fn caps(codec: &str, container: &str) -> Capabilities {
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
        containers: vec![container.into()],
        subtitles: vec![],
        hls_keyframe_exact: true,
    }
}

pub fn start_body(sid: &str, path: &std::path::Path, codec: &str, container: &str) -> Value {
    json!({
        "session_id": sid,
        "path": path.display().to_string(),
        "capabilities": serde_json::to_value(caps(codec, container)).unwrap(),
        "analysis": serde_json::to_value(analysis()).unwrap(),
        "position": 0.0,
    })
}
