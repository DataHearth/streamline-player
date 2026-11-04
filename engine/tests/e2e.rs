mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{body_bytes, caps, id, valid_query};
use engine::config::{Config, HwAccel};
use engine::session::Sessions;
use engine::{AppState, app, hw, jobs::Jobs};
use serde_json::{Value, json};
use tower::ServiceExt;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Env {
    root: PathBuf,
    router: Router,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn have_ffmpeg() -> bool {
    let ok = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        println!("skipped: ffmpeg not on PATH");
    }
    ok
}

fn ffmpeg(args: &[&str]) {
    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "ffmpeg {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn ffprobe(file: &Path, entries: &str) -> Value {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries"])
        .arg(entries)
        .args(["-of", "json"])
        .arg(file)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

const KEYFRAMES: &str = "0,1.5,4,4.2,9,13.7";

fn make_h264_mkv(path: &Path) {
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=1280x720:rate=30:duration=20",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000:duration=20",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-b:v",
        "6M",
        "-maxrate",
        "6M",
        "-bufsize",
        "12M",
        "-pix_fmt",
        "yuv420p",
        "-g",
        "60",
        "-sc_threshold",
        "0",
        "-force_key_frames",
        KEYFRAMES,
        "-c:a",
        "aac",
        "-ac",
        "2",
        path.to_str().unwrap(),
    ]);
}

fn make_hdr10_mkv(path: &Path) {
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=1280x720:rate=30:duration=20",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000:duration=20",
        "-c:v",
        "libx265",
        "-preset",
        "ultrafast",
        "-pix_fmt",
        "yuv420p10le",
        "-x265-params",
        "hdr10=1:colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:log-level=error",
        "-c:a",
        "aac",
        "-ac",
        "2",
        path.to_str().unwrap(),
    ]);
}

async fn env() -> Env {
    let root = std::env::temp_dir().join(format!(
        "engine-e2e-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let cfg = Arc::new(Config {
        secret: Some(common::SECRET.into()),
        hw_accel: HwAccel::None,
        tmp_dir: root.join("live"),
        jobs_dir: root.join("jobs"),
        ..Config::default()
    });
    let hw = Arc::new(hw::probe(&cfg).await);
    let router = app(AppState {
        cfg: cfg.clone(),
        secret: Arc::new(common::SECRET.as_bytes().to_vec()),
        hw: hw.clone(),
        ffmpeg_version: None,
        sessions: Sessions::new(cfg.clone(), hw),
        jobs: Jobs::new(cfg),
    });
    Env { root, router }
}

impl Env {
    async fn send(&self, req: Request<Body>) -> (StatusCode, Vec<u8>) {
        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        (status, body_bytes(res).await)
    }

    async fn api(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut b = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {}", common::SECRET));
        let body = match body {
            Some(v) => {
                b = b.header("content-type", "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let (status, bytes) = self.send(b.body(body).unwrap()).await;
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn start(&self, sid: &str, path: &Path, caps: Value, extra: Value) -> Value {
        let mut body = json!({
            "session_id": sid,
            "path": path.display().to_string(),
            "capabilities": caps,
            "position": 0.0,
        });
        body.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let (status, v) = self.api("POST", "/v1/sessions", Some(body)).await;
        assert_eq!(status, StatusCode::OK, "{v}");
        v
    }

    async fn hls(&self, sid: &str, file: &str) -> Vec<u8> {
        let uri = format!("/hls/{sid}/{file}?{}", valid_query(sid));
        let (status, bytes) = self
            .send(Request::get(uri).body(Body::empty()).unwrap())
            .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{file}: {}",
            String::from_utf8_lossy(&bytes)
        );
        bytes
    }

    async fn bounds(&self, sid: &str) -> Vec<f64> {
        let text = String::from_utf8(self.hls(sid, "media.m3u8").await).unwrap();
        let mut t = 0.0;
        let mut out = vec![];
        for line in text.lines() {
            if let Some(d) = line.strip_prefix("#EXTINF:") {
                out.push(t);
                t += d.trim_end_matches(',').parse::<f64>().unwrap();
            }
        }
        out
    }
}

struct Boxes<'a>(&'a [u8]);

impl<'a> Iterator for Boxes<'a> {
    type Item = ([u8; 4], &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.len() < 8 {
            return None;
        }
        let size = u32::from_be_bytes(self.0[..4].try_into().unwrap()) as usize;
        let size = if size == 0 { self.0.len() } else { size };
        let kind: [u8; 4] = self.0[4..8].try_into().unwrap();
        let body = &self.0[8..size];
        self.0 = &self.0[size..];
        Some((kind, body))
    }
}

fn child<'a>(data: &'a [u8], path: &[&str]) -> Option<&'a [u8]> {
    let (first, rest) = path.split_first()?;
    let (_, body) = Boxes(data).find(|(k, _)| k == first.as_bytes())?;
    if rest.is_empty() {
        Some(body)
    } else {
        child(body, rest)
    }
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes(b[..4].try_into().unwrap())
}

fn be64(b: &[u8]) -> u64 {
    u64::from_be_bytes(b[..8].try_into().unwrap())
}

/// Timescale of the video track in init.mp4, with its track id.
fn video_timescale(init: &[u8]) -> (u32, u32) {
    let moov = child(init, &["moov"]).unwrap();
    for (kind, trak) in Boxes(moov) {
        if &kind != b"trak" {
            continue;
        }
        let hdlr = child(trak, &["mdia", "hdlr"]).unwrap();
        if &hdlr[8..12] != b"vide" {
            continue;
        }
        let tkhd = child(trak, &["tkhd"]).unwrap();
        let track_id = if tkhd[0] == 1 {
            be32(&tkhd[20..])
        } else {
            be32(&tkhd[12..])
        };
        let mdhd = child(trak, &["mdia", "mdhd"]).unwrap();
        let timescale = if mdhd[0] == 1 {
            be32(&mdhd[20..])
        } else {
            be32(&mdhd[12..])
        };
        return (track_id, timescale);
    }
    panic!("no video track in init");
}

fn video_tfdt(seg: &[u8], track_id: u32) -> u64 {
    for (kind, moof) in Boxes(seg) {
        if &kind != b"moof" {
            continue;
        }
        for (kind, traf) in Boxes(moof) {
            if &kind != b"traf" {
                continue;
            }
            let tfhd = child(traf, &["tfhd"]).unwrap();
            if be32(&tfhd[4..]) != track_id {
                continue;
            }
            let tfdt = child(traf, &["tfdt"]).unwrap();
            return if tfdt[0] == 1 {
                be64(&tfdt[4..])
            } else {
                u64::from(be32(&tfdt[4..]))
            };
        }
    }
    panic!("no video tfdt in segment");
}

async fn assert_segment_matches(e: &Env, sid: &str, init: &[u8], bounds: &[f64], i: usize) {
    let (track_id, timescale) = video_timescale(init);
    let seg = e.hls(sid, &format!("seg_{i}.m4s")).await;
    let t = video_tfdt(&seg, track_id) as f64 / f64::from(timescale);
    assert!(
        (t - bounds[i]).abs() < 0.001,
        "seg_{i}: tfdt {t} != boundary {}",
        bounds[i]
    );
}

fn caps_json(codec: &str, container: &str) -> Value {
    serde_json::to_value(caps(codec, container)).unwrap()
}

#[tokio::test]
async fn direct_mkv() {
    if !have_ffmpeg() {
        return;
    }
    let e = env().await;
    let src = e.root.join("in.mkv");
    make_h264_mkv(&src);
    let sid = id(1);
    let r = e
        .start(&sid, &src, caps_json("h264", "mkv"), json!({}))
        .await;
    assert_eq!(r["url_path"], "file");

    let uri = format!("/file/{sid}?{}", valid_query(&sid));
    let (status, bytes) = e
        .send(
            Request::get(uri)
                .header("range", "bytes=100-4195")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    let disk = std::fs::read(&src).unwrap();
    assert_eq!(bytes, disk[100..4196]);
}

#[tokio::test]
async fn remux_segments_match_playlist() {
    if !have_ffmpeg() {
        return;
    }
    let e = env().await;
    let src = e.root.join("in.mkv");
    make_h264_mkv(&src);
    let sid = id(2);
    let r = e
        .start(&sid, &src, caps_json("h264", "mp4"), json!({}))
        .await;
    assert_eq!(r["mode"], "remux");

    let bounds = e.bounds(&sid).await;
    assert!(bounds.len() >= 6, "{bounds:?}");
    let init = e.hls(&sid, "init.mp4").await;
    for i in 0..bounds.len() {
        assert_segment_matches(&e, &sid, &init, &bounds, i).await;
    }
}

#[tokio::test]
async fn remux_after_seek_matches_playlist() {
    if !have_ffmpeg() {
        return;
    }
    let e = env().await;
    let src = e.root.join("in.mkv");
    make_h264_mkv(&src);
    let sid = id(3);
    e.start(&sid, &src, caps_json("h264", "mp4"), json!({}))
        .await;

    let bounds = e.bounds(&sid).await;
    let last = bounds.len() - 1;
    let init = e.hls(&sid, "init.mp4").await;
    assert_segment_matches(&e, &sid, &init, &bounds, 0).await;
    assert_segment_matches(&e, &sid, &init, &bounds, last - 1).await;
    assert_segment_matches(&e, &sid, &init, &bounds, last - 2).await;
}

#[tokio::test]
async fn transcode_cpu_h264() {
    if !have_ffmpeg() {
        return;
    }
    let e = env().await;
    let src = e.root.join("in.mkv");
    make_h264_mkv(&src);
    let sid = id(4);
    let r = e
        .start(
            &sid,
            &src,
            caps_json("h264", "mp4"),
            json!({"max_bitrate": 2_000_000}),
        )
        .await;
    assert_eq!(r["mode"], "transcode");

    let init = e.hls(&sid, "init.mp4").await;
    let seg0 = e.hls(&sid, "seg_0.m4s").await;
    let seg2 = e.hls(&sid, "seg_2.m4s").await;
    let (track_id, timescale) = video_timescale(&init);
    assert_eq!(video_tfdt(&seg0, track_id), 0);
    let t2 = video_tfdt(&seg2, track_id) as f64 / f64::from(timescale);
    assert!((t2 - 12.0).abs() < 0.001, "{t2}");

    let joined = e.root.join("joined.mp4");
    std::fs::write(&joined, [init, seg0].concat()).unwrap();
    let v = ffprobe(&joined, "stream=width,height");
    let s = &v["streams"][0];
    assert!(s["width"].as_u64().unwrap() <= 854, "{v}");
    assert!(s["height"].as_u64().unwrap() <= 480, "{v}");
}

#[tokio::test]
async fn hdr10_tonemapped_cpu() {
    if !have_ffmpeg() {
        return;
    }
    let e = env().await;
    let src = e.root.join("hdr.mkv");
    make_hdr10_mkv(&src);
    let sid = id(5);
    let r = e
        .start(&sid, &src, caps_json("hevc", "mp4"), json!({}))
        .await;
    assert_eq!(r["mode"], "transcode");

    let init = e.hls(&sid, "init.mp4").await;
    let seg0 = e.hls(&sid, "seg_0.m4s").await;
    let joined = e.root.join("joined.mp4");
    std::fs::write(&joined, [init, seg0].concat()).unwrap();
    let v = ffprobe(&joined, "stream=color_transfer");
    assert_eq!(v["streams"][0]["color_transfer"], "bt709", "{v}");
}

#[tokio::test]
async fn analyze_job() {
    if !have_ffmpeg() {
        return;
    }
    let e = env().await;
    let src = e.root.join("in.mkv");
    make_h264_mkv(&src);
    let jid = id(6);
    let (status, _) = e
        .api(
            "POST",
            "/v1/jobs",
            Some(json!({
                "job_id": jid,
                "type": "analyze",
                "priority": "interactive",
                "path": src.display().to_string(),
            })),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let mut status = Value::Null;
    for _ in 0..200 {
        let (_, v) = e.api("GET", &format!("/v1/jobs/{jid}"), None).await;
        if v["state"] == "done" || v["state"] == "failed" {
            status = v;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(status["state"], "done", "{status}");
    let keyframes: Vec<f64> = status["result"]["keyframes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_f64().unwrap())
        .collect();
    for want in [1.5, 4.0, 4.2] {
        assert!(
            keyframes.iter().any(|k| (k - want).abs() < 0.001),
            "{want} not in {keyframes:?}"
        );
    }
}
