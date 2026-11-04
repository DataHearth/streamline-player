mod runner;

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use opentelemetry::KeyValue;
use serde::{Deserialize, Serialize};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tracing::Instrument;

use crate::analysis::{self, Analysis, Hdr};
use crate::caps::Capabilities;
use crate::config::Config;
use crate::decision::{
    AudioAction, DvHandling, HdrOut, Mode, Plan, Reason, SessionParams, VideoAction, VideoCodec,
    decide,
};
use crate::error::ApiError;
use crate::hw::{Backend, Hw};
use crate::playlist;

pub(crate) use runner::OpenStream;
use runner::{Live, Session, State};

pub const RUN_AHEAD_SECS: f64 = 180.0;
pub const RESUME_MARGIN_SECS: f64 = 60.0;
pub const KEEP_BEHIND_SECS: f64 = 180.0;
pub const SEGMENT_WAIT: Duration = Duration::from_secs(30);
pub const REAP_INTERVAL: Duration = Duration::from_secs(60);

const TRANSCODE_RETRY_AFTER: u64 = 10;

#[derive(Debug, Clone, Deserialize)]
pub struct StartRequest {
    pub session_id: String,
    pub path: String,
    pub capabilities: Capabilities,
    #[serde(default)]
    pub analysis: Option<Analysis>,
    #[serde(default)]
    pub position: f64,
    #[serde(default)]
    pub audio_index: Option<u32>,
    #[serde(default)]
    pub burn_in_subtitle: Option<u32>,
    #[serde(default)]
    pub max_bitrate: Option<u64>,
}

/// An absent field keeps the session's current value, an explicit `null`
/// clears it (default audio, no burn-in, original quality).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PatchRequest {
    #[serde(default, deserialize_with = "set_or_clear")]
    pub audio_index: Option<Option<u32>>,
    #[serde(default, deserialize_with = "set_or_clear")]
    pub burn_in_subtitle: Option<Option<u32>>,
    #[serde(default, deserialize_with = "set_or_clear")]
    pub max_bitrate: Option<Option<u64>>,
    #[serde(default)]
    pub position: Option<f64>,
}

fn set_or_clear<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VideoOut {
    pub action: &'static str,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub hdr: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dv_profile: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioOut {
    pub action: &'static str,
    pub codec: String,
    pub channels: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionResponse {
    pub mode: Mode,
    pub video: VideoOut,
    pub audio: AudioOut,
    pub video_reasons: Vec<Reason>,
    pub audio_reasons: Vec<Reason>,
    pub hw: String,
    pub duration: f64,
    pub url_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub mode: Mode,
    pub hw: String,
    pub video: VideoOut,
    pub audio: AudioOut,
    pub speed: Option<f64>,
    pub bitrate: Option<u64>,
    pub last_fetch: Option<u64>,
    pub bytes_served: u64,
}

#[derive(Debug)]
pub enum Served {
    Text(String),
    File(PathBuf),
}

#[derive(Clone)]
pub struct Sessions {
    inner: Arc<Inner>,
}

struct Inner {
    cfg: Arc<Config>,
    hw: Arc<Hw>,
    permits: Arc<Semaphore>,
    map: Mutex<HashMap<String, Arc<Session>>>,
    reap_interval: Duration,
}

fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(crate) fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn is_transcode(mode: Mode) -> bool {
    matches!(mode, Mode::AudioTranscode | Mode::Transcode)
}

pub fn backend_name(b: Backend) -> &'static str {
    match b {
        Backend::Vaapi => "vaapi",
        Backend::Nvenc => "nvenc",
        Backend::Qsv => "qsv",
        Backend::Amf => "amf",
        Backend::Videotoolbox => "videotoolbox",
        Backend::Cpu => "cpu",
    }
}

fn hdr_name(h: Hdr) -> &'static str {
    match h {
        Hdr::Sdr => "sdr",
        Hdr::Hdr10 => "hdr10",
        Hdr::Hlg => "hlg",
    }
}

fn codec_name(c: VideoCodec) -> &'static str {
    match c {
        VideoCodec::H264 => "h264",
        VideoCodec::Hevc => "hevc",
        VideoCodec::Av1 => "av1",
    }
}

pub(super) fn hw_name(plan: &Plan, hw: &Hw) -> &'static str {
    match plan.video {
        VideoAction::Transcode { .. } => backend_name(hw.backend),
        VideoAction::Copy { .. } => "none",
    }
}

pub(super) fn video_out(plan: &Plan, a: &Analysis) -> VideoOut {
    let src = a.video.iter().find(|v| v.index == plan.video_index);
    match plan.video {
        VideoAction::Copy { dv, .. } => {
            let (hdr, dv_profile) = match (src.and_then(|v| v.dv.as_ref()), dv) {
                (Some(d), DvHandling::Keep) => ("dolby_vision", Some(d.profile)),
                (Some(_), DvHandling::P7ToP81) => ("dolby_vision", Some(8)),
                _ => (hdr_name(src.map_or(Hdr::Sdr, |v| v.hdr)), None),
            };
            VideoOut {
                action: "copy",
                codec: src.map(|v| v.codec.clone()).unwrap_or_default(),
                width: src.map_or(0, |v| v.width),
                height: src.map_or(0, |v| v.height),
                hdr,
                dv_profile,
            }
        }
        VideoAction::Transcode {
            codec,
            width,
            height,
            hdr_out,
            ..
        } => VideoOut {
            action: "transcode",
            codec: codec_name(codec).to_owned(),
            width,
            height,
            hdr: match hdr_out {
                HdrOut::Keep(h) => hdr_name(h),
                HdrOut::Tonemap => "sdr",
            },
            dv_profile: None,
        },
    }
}

pub(super) fn audio_out(plan: &Plan, a: &Analysis) -> AudioOut {
    match plan.audio {
        AudioAction::Copy => {
            let src = plan
                .audio_index
                .and_then(|i| a.audio.iter().find(|s| s.index == i));
            AudioOut {
                action: "copy",
                codec: src.map(|s| s.codec.clone()).unwrap_or_default(),
                channels: src.map_or(0, |s| s.channels),
            }
        }
        AudioAction::Transcode {
            codec, channels, ..
        } => AudioOut {
            action: "transcode",
            codec: codec.to_owned(),
            channels,
        },
    }
}

fn bandwidth(plan: &Plan, a: &Analysis) -> u64 {
    match (plan.video.clone(), &plan.audio) {
        (VideoAction::Transcode { bitrate, .. }, AudioAction::Transcode { bitrate: ab, .. }) => {
            bitrate + ab
        }
        (VideoAction::Transcode { bitrate, .. }, AudioAction::Copy) => bitrate,
        _ => a.bit_rate,
    }
}

fn target_bitrate(plan: &Plan) -> Option<u64> {
    match plan.video {
        VideoAction::Transcode { bitrate, .. } => Some(bitrate),
        VideoAction::Copy { .. } => None,
    }
}

fn live_for(plan: &Plan, a: &Analysis, hw: &Hw, speed: Option<Arc<AtomicU64>>) -> Live {
    Live {
        mode: plan.mode,
        hw: hw_name(plan, hw),
        video: video_out(plan, a),
        audio: audio_out(plan, a),
        bitrate: target_bitrate(plan),
        speed,
    }
}

fn response(plan: &Plan, a: &Analysis, hw: &Hw) -> SessionResponse {
    SessionResponse {
        mode: plan.mode,
        video: video_out(plan, a),
        audio: audio_out(plan, a),
        video_reasons: plan.video_reasons.clone(),
        audio_reasons: plan.audio_reasons.clone(),
        hw: hw_name(plan, hw).to_owned(),
        duration: a.duration,
        url_path: if plan.mode == Mode::Direct {
            "file"
        } else {
            "master.m3u8"
        }
        .to_owned(),
    }
}

fn build_state(
    caps: Capabilities,
    analysis: Arc<Analysis>,
    plan: Plan,
    hw: &Hw,
    params: SessionParams,
    permit: Option<OwnedSemaphorePermit>,
    position: f64,
) -> State {
    let hls = plan.mode != Mode::Direct;
    let bounds = if hls {
        playlist::boundaries(&plan, &analysis)
    } else {
        Vec::new()
    };
    let (master, media) = if hls {
        (
            playlist::master_playlist(&plan, &analysis, bandwidth(&plan, &analysis)),
            playlist::media_playlist(&bounds, analysis.duration),
        )
    } else {
        (String::new(), String::new())
    };
    let fetched_t = if hls {
        bounds[playlist::segment_for(&bounds, position) as usize]
    } else {
        0.0
    };
    State {
        closed: false,
        next_run_id: 0,
        caps,
        analysis,
        plan,
        hw: hw.clone(),
        params,
        bounds,
        master,
        media,
        completed: BTreeSet::new(),
        fetched_t,
        run: None,
        permit,
    }
}

impl Sessions {
    pub fn new(cfg: Arc<Config>, hw: Arc<Hw>) -> Self {
        Self::with_reap_interval(cfg, hw, REAP_INTERVAL)
    }

    pub fn with_reap_interval(cfg: Arc<Config>, hw: Arc<Hw>, reap_interval: Duration) -> Self {
        Self {
            inner: Arc::new(Inner {
                permits: Arc::new(Semaphore::new(cfg.max_transcodes)),
                cfg,
                hw,
                map: Mutex::new(HashMap::new()),
                reap_interval,
            }),
        }
    }

    fn get(&self, id: &str) -> Result<Arc<Session>, ApiError> {
        if !valid_id(id) {
            return Err(ApiError::invalid_request("session id must be 32 hex chars"));
        }
        self.inner
            .map
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("unknown session"))
    }

    fn permit_for(&self, mode: Mode) -> Result<Option<OwnedSemaphorePermit>, ApiError> {
        if !is_transcode(mode) {
            return Ok(None);
        }
        self.inner
            .permits
            .clone()
            .try_acquire_owned()
            .map(Some)
            .map_err(|_| ApiError::transcode_limit(TRANSCODE_RETRY_AFTER))
    }

    pub async fn start(&self, r: StartRequest) -> Result<SessionResponse, ApiError> {
        let span = tracing::info_span!("engine.start", session = %r.session_id);
        let reply = self.start_inner(r).instrument(span).await?;
        crate::telemetry::metrics().sessions.add(
            1,
            &[
                KeyValue::new(
                    "mode",
                    serde_json::to_value(reply.mode)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                ),
                KeyValue::new("hw", reply.hw.clone()),
            ],
        );
        Ok(reply)
    }

    async fn start_inner(&self, r: StartRequest) -> Result<SessionResponse, ApiError> {
        if !valid_id(&r.session_id) {
            return Err(ApiError::invalid_request("session_id must be 32 hex chars"));
        }
        if self.inner.map.lock().unwrap().contains_key(&r.session_id) {
            return Err(ApiError::session_exists());
        }
        let cfg = &self.inner.cfg;
        let input = cfg.path_map.rewrite(&r.path);
        if !tokio::fs::metadata(&input).await.is_ok_and(|m| m.is_file()) {
            return Err(ApiError::file_not_found(format!(
                "no such file: {}",
                input.display()
            )));
        }
        let analysis = match r.analysis.filter(analysis::is_current) {
            Some(a) => a,
            None => analysis::run_analyze(&cfg.ffprobe(), &input)
                .await
                .map_err(|e| ApiError::ffmpeg_unavailable(e.to_string()))?,
        };
        let params = SessionParams {
            audio_index: r.audio_index,
            burn_in_subtitle: r.burn_in_subtitle,
            max_bitrate: r.max_bitrate,
        };
        let plan = decide(&r.capabilities, &analysis, &params, &self.inner.hw.encoders)?;
        let permit = self.permit_for(plan.mode)?;

        let hw = &self.inner.hw;
        let reply = response(&plan, &analysis, hw);
        let analysis = Arc::new(analysis);
        let live = live_for(&plan, &analysis, hw, None);
        let hls = plan.mode != Mode::Direct;
        let state = build_state(
            r.capabilities,
            analysis,
            plan,
            hw,
            params,
            permit,
            r.position,
        );
        let start_index = playlist::segment_for(&state.bounds, r.position);

        let session = Arc::new(Session::new(
            r.session_id.clone(),
            cfg.tmp_dir.join(&r.session_id),
            input,
            cfg.clone(),
            state,
            live,
        ));
        {
            let mut map = self.inner.map.lock().unwrap();
            if map.contains_key(&r.session_id) {
                return Err(ApiError::session_exists());
            }
            map.insert(r.session_id.clone(), session.clone());
        }
        if hls {
            let mut st = session.state.lock().await;
            if let Err(e) = session.begin(&mut st, start_index).await {
                st.closed = true;
                drop(st);
                self.inner.map.lock().unwrap().remove(&r.session_id);
                let _ = tokio::fs::remove_dir_all(&session.dir).await;
                return Err(e);
            }
        }
        Ok(reply)
    }

    pub async fn patch(&self, id: &str, r: PatchRequest) -> Result<SessionResponse, ApiError> {
        let s = self.get(id)?;
        s.touch();
        let mut st = s.state.lock().await;
        if st.closed {
            return Err(ApiError::not_found("unknown session"));
        }
        let params = SessionParams {
            audio_index: r.audio_index.unwrap_or(st.params.audio_index),
            burn_in_subtitle: r.burn_in_subtitle.unwrap_or(st.params.burn_in_subtitle),
            max_bitrate: r.max_bitrate.unwrap_or(st.params.max_bitrate),
        };
        let position = r.position.unwrap_or(st.fetched_t);
        let analysis = st.analysis.clone();
        let plan = decide(&st.caps, &analysis, &params, &self.inner.hw.encoders)?;
        let permit = match (is_transcode(plan.mode), st.permit.is_some()) {
            (true, false) => self.permit_for(plan.mode)?,
            (true, true) => st.permit.take(),
            (false, _) => None,
        };

        s.epoch.fetch_add(1, Ordering::SeqCst);
        s.stop_run(&mut st).await;
        let hw = &self.inner.hw;
        let reply = response(&plan, &analysis, hw);
        let live = live_for(&plan, &analysis, hw, None);
        let fresh = build_state(
            st.caps.clone(),
            analysis,
            plan,
            hw,
            params,
            permit,
            position,
        );
        let hls = fresh.plan.mode != Mode::Direct;
        let start_index = playlist::segment_for(&fresh.bounds, position);
        let next_run_id = st.next_run_id;
        *st = State {
            next_run_id,
            ..fresh
        };
        *s.live.lock().unwrap() = live;
        let _ = tokio::fs::remove_dir_all(&s.dir).await;
        if hls {
            s.begin(&mut st, start_index).await?;
        }
        Ok(reply)
    }

    /// The teardown runs detached: a handler dropped mid-delete (the client
    /// hung up) must not leave ffmpeg, the transcode permit or the tmp dir behind.
    pub async fn delete(&self, id: &str) -> Result<(), ApiError> {
        let s = self.get(id)?;
        self.inner.map.lock().unwrap().remove(id);
        let teardown = tokio::spawn(async move {
            let mut st = s.state.lock().await;
            st.closed = true;
            s.stop_run(&mut st).await;
            st.permit = None;
            drop(st);
            crate::telemetry::metrics()
                .session_duration
                .record(s.started.elapsed().as_secs_f64(), &[]);
            let _ = tokio::fs::remove_dir_all(&s.dir).await;
        });
        teardown
            .await
            .map_err(|e| ApiError::ffmpeg_failed(format!("session teardown failed: {e}")))
    }

    pub(crate) fn open_stream(&self, id: &str) -> Option<OpenStream> {
        self.get(id).ok().map(|s| s.open_stream())
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        let map = self.inner.map.lock().unwrap();
        let mut out: Vec<SessionInfo> = map
            .values()
            .map(|s| {
                let live = s.live.lock().unwrap();
                let last_fetch = s.last_fetch.load(Ordering::Relaxed);
                SessionInfo {
                    session_id: s.id.clone(),
                    mode: live.mode,
                    hw: live.hw.to_owned(),
                    video: live.video.clone(),
                    audio: live.audio.clone(),
                    speed: live.speed(),
                    bitrate: live.bitrate,
                    last_fetch: (last_fetch != 0).then_some(last_fetch),
                    bytes_served: s.bytes.load(Ordering::Relaxed),
                }
            })
            .collect();
        out.sort_by(|a, b| a.session_id.cmp(&b.session_id));
        out
    }

    pub async fn fetch_hls(&self, id: &str, file: &str) -> Result<Served, ApiError> {
        let s = self.get(id)?;
        s.touch();
        s.stamp_fetch();
        s.serve(file).await
    }

    pub async fn direct_path(&self, id: &str) -> Result<PathBuf, ApiError> {
        let s = self.get(id)?;
        s.touch();
        s.stamp_fetch();
        let st = s.state.lock().await;
        if st.closed || st.plan.mode != Mode::Direct {
            return Err(ApiError::not_found("session is not a direct session"));
        }
        Ok(s.input.clone())
    }

    pub fn add_bytes(&self, id: &str, n: u64) {
        if let Ok(s) = self.get(id) {
            s.bytes.fetch_add(n, Ordering::Relaxed);
            s.touch();
        }
    }

    pub fn spawn_reaper(&self) -> JoinHandle<()> {
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(this.inner.reap_interval).await;
                let timeout = this.inner.cfg.orphan_timeout;
                let stale: Vec<String> = this
                    .inner
                    .map
                    .lock()
                    .unwrap()
                    .values()
                    .filter(|s| s.idle() > timeout)
                    .map(|s| s.id.clone())
                    .collect();
                for id in stale {
                    let _ = this.delete(&id).await;
                }
            }
        })
    }

    pub async fn shutdown(&self) {
        let ids: Vec<String> = self.list().into_iter().map(|i| i.session_id).collect();
        for id in ids {
            let _ = self.delete(&id).await;
        }
    }

    pub fn sweep_tmp(&self) {
        let Ok(entries) = std::fs::read_dir(&self.inner.cfg.tmp_dir) else {
            return;
        };
        for entry in entries.flatten() {
            if !entry.file_name().to_str().is_some_and(valid_id) {
                continue;
            }
            let path = entry.path();
            let _ = if path.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PatchRequest;

    #[test]
    fn patch_distinguishes_absent_null_and_value() {
        let r: PatchRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(r.audio_index, None);
        assert_eq!(r.burn_in_subtitle, None);
        assert_eq!(r.max_bitrate, None);
        assert_eq!(r.position, None);

        let r: PatchRequest = serde_json::from_str(
            r#"{"audio_index":null,"burn_in_subtitle":null,"max_bitrate":null}"#,
        )
        .unwrap();
        assert_eq!(r.audio_index, Some(None));
        assert_eq!(r.burn_in_subtitle, Some(None));
        assert_eq!(r.max_bitrate, Some(None));

        let r: PatchRequest = serde_json::from_str(
            r#"{"audio_index":2,"burn_in_subtitle":3,"max_bitrate":4,"position":5.5}"#,
        )
        .unwrap();
        assert_eq!(r.audio_index, Some(Some(2)));
        assert_eq!(r.burn_in_subtitle, Some(Some(3)));
        assert_eq!(r.max_bitrate, Some(Some(4)));
        assert_eq!(r.position, Some(5.5));
    }
}
