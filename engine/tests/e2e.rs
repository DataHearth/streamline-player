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

fn ffmpeg_gate(present: bool, ci: bool) -> bool {
    assert!(
        present || !ci,
        "ffmpeg is not on PATH but CI is set: the e2e suite must not pass without it"
    );
    if !present {
        println!("skipped: ffmpeg not on PATH");
    }
    present
}

fn have_ffmpeg() -> bool {
    let present = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success());
    ffmpeg_gate(present, std::env::var_os("CI").is_some())
}

#[test]
#[should_panic(expected = "CI is set")]
fn missing_ffmpeg_panics_in_ci() {
    ffmpeg_gate(false, true);
}

#[test]
fn missing_ffmpeg_skips_outside_ci() {
    assert!(!ffmpeg_gate(false, false));
    assert!(ffmpeg_gate(true, true));
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
    ffprobe_stream(file, "v:0", entries)
}

fn ffprobe_stream(file: &Path, select: &str, entries: &str) -> Value {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", select, "-show_entries"])
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

fn make_surround_flac_mkv(path: &Path) {
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
        "-pix_fmt",
        "yuv420p",
        "-g",
        "60",
        "-sc_threshold",
        "0",
        "-force_key_frames",
        KEYFRAMES,
        "-af",
        "aformat=channel_layouts=7.1",
        "-c:a",
        "flac",
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

fn make_late_video_mkv(path: &Path) {
    ffmpeg(&[
        "-itsoffset",
        "0.5",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=640x360:rate=30:duration=20",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000:duration=20",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
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

async fn first_video_pts(e: &Env, sid: &str, init: &[u8], i: usize) -> f64 {
    let mut media = init.to_vec();
    media.extend(e.hls(sid, &format!("seg_{i}.m4s")).await);
    let file = e.root.join(format!("{sid}-{i}.mp4"));
    std::fs::write(&file, media).unwrap();
    let probe = ffprobe(&file, "packet=pts_time");
    probe["packets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["pts_time"].as_str().unwrap().parse::<f64>().unwrap())
        .fold(f64::INFINITY, f64::min)
}

// A restart may drop one leading B-frame; landing a GOP early is far more off.
const RESTART_SLACK_SECS: f64 = 0.05;

async fn assert_restart_matches_in_order(
    e: &Env,
    src: &Path,
    caps: Value,
    extra: Value,
    k: usize,
    n: u8,
) {
    let a = id(n);
    e.start(&a, src, caps.clone(), extra.clone()).await;
    let bounds = e.bounds(&a).await;
    let init = e.hls(&a, "init.mp4").await;
    for i in 0..k {
        e.hls(&a, &format!("seg_{i}.m4s")).await;
    }
    let in_order = first_video_pts(e, &a, &init, k).await;
    let (status, _) = e.api("DELETE", &format!("/v1/sessions/{a}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let b = id(n + 1);
    let mut extra = extra;
    extra["position"] = json!(bounds[k]);
    e.start(&b, src, caps, extra).await;
    let own_init = e.hls(&b, "init.mp4").await;
    let restarted = first_video_pts(e, &b, &own_init, k).await;
    assert!(
        (in_order - restarted).abs() < RESTART_SLACK_SECS,
        "seg_{k}: restart's earliest video pts is {restarted}, in order at {in_order}"
    );
    // A player playing on through a restart keeps the init it already has.
    let kept = first_video_pts(e, &b, &init, k).await;
    assert!(
        (in_order - kept).abs() < RESTART_SLACK_SECS,
        "seg_{k}: with the first run's init the restart reads {kept}, in order at {in_order}"
    );
}

#[tokio::test]
async fn remux_restart_with_late_video_matches_in_order() {
    if !have_ffmpeg() {
        return;
    }
    let e = env().await;
    let src = e.root.join("late.mkv");
    make_late_video_mkv(&src);
    assert_restart_matches_in_order(&e, &src, caps_json("h264", "mp4"), json!({}), 3, 20).await;
}

#[tokio::test]
async fn transcode_restart_with_late_video_matches_in_order() {
    if !have_ffmpeg() {
        return;
    }
    let e = env().await;
    let src = e.root.join("late.mkv");
    make_late_video_mkv(&src);
    let extra = json!({"max_bitrate": 2_000_000});
    assert_restart_matches_in_order(&e, &src, caps_json("h264", "mp4"), extra, 2, 40).await;
}

#[tokio::test]
async fn remux_restart_on_corpus_flac_matches_in_order() {
    if !have_ffmpeg() {
        return;
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/10-flac-51.mkv");
    if !src.exists() {
        println!("skipped: {} not present", src.display());
        return;
    }
    let e = env().await;
    let mut caps = caps_json("hevc", "mp4");
    caps["video"][0]["profiles"] = json!(["main"]);
    caps["audio"]["codecs"] = json!(["flac"]);
    assert_restart_matches_in_order(&e, &src, caps, json!({}), 1, 30).await;
}

async fn joined_segment(e: &Env, sid: &str, init: &[u8], i: usize) -> PathBuf {
    let mut media = init.to_vec();
    media.extend(e.hls(sid, &format!("seg_{i}.m4s")).await);
    let file = e.root.join(format!("{sid}-{i}-joined.mp4"));
    std::fs::write(&file, media).unwrap();
    file
}

fn min_pts(file: &Path, stream: &str) -> f64 {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", stream])
        .args(["-show_entries", "packet=pts_time", "-of", "csv=p=0"])
        .arg(file)
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse::<f64>().ok())
        .fold(f64::INFINITY, f64::min)
}

fn dovi_record(file: &Path) -> Value {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0"])
        .args(["-show_entries", "stream_side_data", "-of", "json"])
        .arg(file)
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    v["streams"][0]["side_data_list"]
        .as_array()
        .and_then(|l| {
            l.iter()
                .find(|d| d["side_data_type"] == "DOVI configuration record")
        })
        .cloned()
        .unwrap_or(Value::Null)
}

struct DvRun {
    bounds: Vec<f64>,
    in_order: PathBuf,
    restarted: PathBuf,
}

async fn dv_p7_run(e: &Env, src: &Path, dv_profile: u8, n: u8) -> DvRun {
    let mut caps = caps_json("hevc", "mp4");
    caps["video"][0]["profiles"] = json!(["main", "main_10"]);
    caps["hdr"] = json!({"formats": ["hdr10", "dolby_vision"], "dv_profiles": [dv_profile]});
    caps["audio"]["codecs"] = json!(["ac3"]);

    let a = id(n);
    e.start(&a, src, caps.clone(), json!({})).await;
    let bounds = e.bounds(&a).await;
    let init = e.hls(&a, "init.mp4").await;
    let k = 2;
    for i in 0..k {
        e.hls(&a, &format!("seg_{i}.m4s")).await;
    }
    let in_order = joined_segment(e, &a, &init, k).await;
    let (status, _) = e.api("DELETE", &format!("/v1/sessions/{a}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let b = id(n + 1);
    e.start(&b, src, caps, json!({"position": bounds[k]})).await;
    let init = e.hls(&b, "init.mp4").await;
    let restarted = joined_segment(e, &b, &init, k).await;
    DvRun {
        bounds,
        in_order,
        restarted,
    }
}

async fn dv_p7_converts_to_p81(clip: &str, n: u8) {
    if !have_ffmpeg() {
        return;
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("corpus")
        .join(clip);
    if !src.exists() {
        println!("skipped: {} not present", src.display());
        return;
    }
    let dovi_tool = Command::new("dovi_tool")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !dovi_tool {
        println!("skipped: dovi_tool not on PATH");
        return;
    }
    let _serial = DV_LOCK.lock().await;
    let e = env().await;
    let converted = dv_p7_run(&e, &src, 8, n).await;
    let untouched = dv_p7_run(&e, &src, 7, n + 2).await;
    let k = 2;

    for file in [&converted.in_order, &converted.restarted] {
        let record = dovi_record(file);
        assert_eq!(record["dv_profile"], 8, "{record}");
        assert_eq!(record["dv_bl_signal_compatibility_id"], 1, "{record}");
    }

    let video = |r: &DvRun| (min_pts(&r.in_order, "v:0"), min_pts(&r.restarted, "v:0"));
    let audio = |r: &DvRun| (min_pts(&r.in_order, "a:0"), min_pts(&r.restarted, "a:0"));
    let (v_in, v_re) = video(&converted);
    assert!(
        (v_re - converted.bounds[k]).abs() < RESTART_SLACK_SECS,
        "restart video pts {v_re} != boundary {}",
        converted.bounds[k]
    );
    let (plain_v_in, plain_v_re) = video(&untouched);
    assert!(
        (v_in - plain_v_in).abs() < RESTART_SLACK_SECS,
        "in-order video pts {v_in}, plain remux {plain_v_in}"
    );
    assert!((v_re - plain_v_re).abs() < RESTART_SLACK_SECS);

    // The converted run reads audio through a second input, which can land a
    // frame or two off the plain remux's.
    let (a_in, a_re) = audio(&converted);
    let (plain_a_in, plain_a_re) = audio(&untouched);
    assert!(
        (a_in - plain_a_in).abs() < AUDIO_SLACK_SECS,
        "audio {a_in} vs {plain_a_in}"
    );
    assert!(
        (a_re - plain_a_re).abs() < AUDIO_SLACK_SECS,
        "audio {a_re} vs {plain_a_re}"
    );
}

const AUDIO_SLACK_SECS: f64 = 0.2;

static DV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(target_os = "linux")]
fn own_children(names: &[&str]) -> Vec<String> {
    let me = std::process::id().to_string();
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some((head, tail)) = stat.rsplit_once(") ") else {
            continue;
        };
        let comm = head.split_once(" (").map_or("", |(_, c)| c);
        let ppid = tail.split(' ').nth(1).unwrap_or("");
        if ppid == me && names.contains(&comm) {
            found.push(comm.to_owned());
        }
    }
    found
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn dv_p7_stop_kills_the_whole_pipeline() {
    if !have_ffmpeg() {
        return;
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/02-dv-p7-fel.mkv");
    if !src.exists() || Command::new("dovi_tool").arg("--version").output().is_err() {
        println!("skipped: P7 clip or dovi_tool not present");
        return;
    }
    let _serial = DV_LOCK.lock().await;
    let e = env().await;
    let mut caps = caps_json("hevc", "mp4");
    caps["hdr"] = json!({"formats": ["hdr10", "dolby_vision"], "dv_profiles": [8]});
    caps["audio"]["codecs"] = json!(["ac3"]);
    let sid = id(70);
    e.start(&sid, &src, caps, json!({})).await;
    e.hls(&sid, "init.mp4").await;
    assert!(
        own_children(&["dovi_tool"]).len() <= 1,
        "only this session may own a dovi_tool"
    );

    let (status, _) = e.api("DELETE", &format!("/v1/sessions/{sid}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    for _ in 0..50 {
        if own_children(&["dovi_tool"]).is_empty() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("dovi_tool outlived its session");
}

#[tokio::test]
async fn dv_p7_fel_converts_to_p81() {
    dv_p7_converts_to_p81("02-dv-p7-fel.mkv", 50).await;
}

#[tokio::test]
async fn dv_p7_mel_converts_to_p81() {
    dv_p7_converts_to_p81("19-dv-p7-mel.mkv", 60).await;
}

#[tokio::test]
async fn audio_transcode_eac3_keeps_video_copy() {
    if !have_ffmpeg() {
        return;
    }
    let e = env().await;
    let src = e.root.join("surround.mkv");
    make_surround_flac_mkv(&src);
    let mut caps = caps_json("h264", "mp4");
    caps["audio"]["codecs"] = json!(["eac3"]);
    let sid = id(7);
    let r = e.start(&sid, &src, caps, json!({})).await;
    assert_eq!(r["mode"], "audio_transcode", "{r}");

    let bounds = e.bounds(&sid).await;
    let init = e.hls(&sid, "init.mp4").await;
    assert_segment_matches(&e, &sid, &init, &bounds, 0).await;

    let joined = joined_segment(&e, &sid, &init, 0).await;
    let a = ffprobe_stream(&joined, "a:0", "stream=codec_name,channels");
    assert_eq!(a["streams"][0]["codec_name"], "eac3", "{a}");
    assert_eq!(a["streams"][0]["channels"], 6, "{a}");
    let v = ffprobe(&joined, "stream=codec_name");
    assert_eq!(v["streams"][0]["codec_name"], "h264", "{v}");
}

#[tokio::test]
async fn audio_transcode_restart_starts_audio_with_the_keyframe() {
    if !have_ffmpeg() {
        return;
    }
    let e = env().await;
    let src = e.root.join("surround.mkv");
    make_surround_flac_mkv(&src);
    let mut caps = caps_json("h264", "mp4");
    caps["audio"]["codecs"] = json!(["eac3"]);
    let first = id(80);
    e.start(&first, &src, caps.clone(), json!({})).await;
    let bounds = e.bounds(&first).await;
    e.api("DELETE", &format!("/v1/sessions/{first}"), None)
        .await;

    let sid = id(81);
    e.start(&sid, &src, caps, json!({"position": bounds[2]}))
        .await;
    let init = e.hls(&sid, "init.mp4").await;
    let joined = joined_segment(&e, &sid, &init, 2).await;
    let (video, audio) = (min_pts(&joined, "v:0"), min_pts(&joined, "a:0"));
    // The transcoded audio is cut at -ss: aiming deep into the GOP left it starting that late.
    assert!(
        (audio - video).abs() < 0.03,
        "audio starts at {audio}, video at {video}"
    );
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
    let spacing =
        (video_tfdt(&seg2, track_id) - video_tfdt(&seg0, track_id)) as f64 / f64::from(timescale);
    assert!((spacing - 12.0).abs() < 0.001, "{spacing}");

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

/// A random tune per seed, a new note every third of a second: same seed, same audio. Steady
/// noise won't do, its fingerprints all look alike. `segments` is (seed, seconds).
fn make_episode(path: &Path, segments: &[(u32, u32)]) {
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args(["-nostdin", "-y", "-hide_banner", "-loglevel", "error"]);
    for (seed, secs) in segments {
        let note = format!("floor(abs(mod(sin(floor(t*3+1)*{seed}*12.9898)*43758.5453\\,24)))");
        cmd.args(["-f", "lavfi", "-i"]).arg(format!(
            "aevalsrc=exprs=sin(2*PI*t*220*pow(2\\,{note}/12)):d={secs}:s=44100"
        ));
    }
    let inputs: String = (0..segments.len()).map(|i| format!("[{i}:a]")).collect();
    cmd.arg("-filter_complex")
        .arg(format!("{inputs}concat=n={}:v=0:a=1[a]", segments.len()))
        .args(["-map", "[a]", "-c:a", "flac"])
        .arg(path);
    assert!(cmd.status().unwrap().success());
}

#[tokio::test]
async fn intro_job_finds_the_shared_intro_and_credits_of_a_season() {
    if !have_ffmpeg() {
        return;
    }
    let e = env().await;
    let (a, b, broken) = (
        e.root.join("a.mkv"),
        e.root.join("b.mkv"),
        e.root.join("gone.mkv"),
    );
    make_episode(&a, &[(11, 5), (1, 30), (12, 90), (2, 20), (13, 5)]);
    make_episode(&b, &[(21, 20), (1, 30), (22, 80), (2, 20)]);
    let paths: Vec<String> = [&a, &broken, &b]
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    let jid = id(7);
    let (status, _) = e
        .api(
            "POST",
            "/v1/jobs",
            Some(json!({ "job_id": jid, "type": "intro", "priority": "interactive", "paths": paths })),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let mut status = Value::Null;
    for _ in 0..600 {
        let (_, v) = e.api("GET", &format!("/v1/jobs/{jid}"), None).await;
        if v["state"] == "done" || v["state"] == "failed" {
            status = v;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(status["state"], "done", "{status}");
    let result = &status["result"];
    let near = |v: &Value, want: f64| {
        let got = v.as_f64().unwrap();
        assert!(
            (got - want).abs() < 3.0,
            "{got} is not about {want}: {result}"
        );
    };
    assert_eq!(result[0]["source"], "fingerprint", "{result}");
    near(&result[0]["intro"][0], 5.0);
    near(&result[0]["intro"][1], 35.0);
    near(&result[0]["credits"][0], 125.0);
    near(&result[0]["credits"][1], 145.0);
    assert_eq!(result[1]["intro"], Value::Null, "{result}");
    near(&result[2]["intro"][0], 20.0);
    near(&result[2]["credits"][0], 130.0);
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

#[tokio::test]
async fn decisions_preview_every_option_without_starting_anything() {
    if !have_ffmpeg() {
        return;
    }
    let e = env().await;
    let src = e.root.join("in.mkv");
    make_h264_mkv(&src);
    let (status, v) = e
        .api(
            "POST",
            "/v1/decisions",
            Some(json!({
                "path": src.display().to_string(),
                "capabilities": caps_json("h264", "mkv"),
                "options": [{}, {"max_bitrate": 100_000}, {"audio_index": 99}],
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert!(v["bit_rate"].as_u64().unwrap() > 0, "{v}");

    let original = &v["decisions"][0];
    assert_eq!(original["mode"], "direct", "{v}");
    assert_eq!(original["video"]["action"], "copy");
    assert_eq!(original["needs_slot"], false);
    assert_eq!(original["bandwidth"], v["bit_rate"]);
    assert!(original.get("url_path").is_none());

    let capped = &v["decisions"][1];
    assert_eq!(capped["mode"], "transcode", "{v}");
    assert_eq!(capped["needs_slot"], true);
    assert_eq!(capped["video_reasons"][0]["code"], "bitrate_cap");
    assert_eq!(capped["video_reasons"][0]["text"], "capped at 0.1 Mbit/s");
    assert!(
        capped["bandwidth"].as_u64().unwrap() < original["bandwidth"].as_u64().unwrap(),
        "{v}"
    );

    assert_eq!(v["decisions"][2]["error"]["code"], "invalid_request", "{v}");

    let (_, list) = e.api("GET", "/v1/sessions", None).await;
    assert_eq!(list["sessions"], json!([]));
}

#[tokio::test]
async fn decisions_answer_for_a_missing_file_and_refuse_too_many_options() {
    let e = env().await;
    let (status, v) = e
        .api(
            "POST",
            "/v1/decisions",
            Some(json!({
                "path": e.root.join("gone.mkv").display().to_string(),
                "capabilities": caps_json("h264", "mkv"),
                "options": [{}],
            })),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{v}");
    assert_eq!(v["code"], "file_not_found");

    let src = e.root.join("present.mkv");
    std::fs::write(&src, b"not read: too many options are refused first").unwrap();
    let (status, v) = e
        .api(
            "POST",
            "/v1/decisions",
            Some(json!({
                "path": src.display().to_string(),
                "capabilities": caps_json("h264", "mkv"),
                "options": vec![json!({}); 33],
            })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["code"], "invalid_request");
}
