//! The player on Linux desktop: libmpv behind the commands and events of the Android plugin
//! (PlayerPlugin.kt), so the page drives both alike.

use crate::{
    mpv::{Event, Mpv},
    view,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{Manager, Runtime, State, Webview, ipc::Channel};

const POSITION_INTERVAL: Duration = Duration::from_millis(500);
const READS_WINDOW: Duration = Duration::from_secs(2);
const LOAD_TIMEOUT: Duration = Duration::from_secs(60);
/// How long the watcher waits for an mpv event before sampling state anyway, in seconds.
const TICK: f64 = 0.1;

type Done = Result<(), String>;

pub struct Player {
    mpv: Arc<Mpv>,
    listeners: Mutex<HashMap<String, Vec<Channel<Value>>>>,
    loading: Mutex<bool>,
    loaded: Condvar,
    direct: AtomicBool,
    fonts: PathBuf,
    subtitles: PathBuf,
}

impl Player {
    fn emit(&self, event: &str, payload: Value) {
        if let Some(channels) = self.listeners.lock().unwrap().get(event) {
            for c in channels {
                let _ = c.send(payload.clone());
            }
        }
    }

    fn emit_position(&self) {
        let Some(position) = self.mpv.number("time-pos") else {
            return;
        };
        self.emit(
            "position",
            json!({
                "position": position,
                "buffered": self.mpv.number("demuxer-cache-time").unwrap_or(position),
                "duration": self.mpv.number("duration").unwrap_or(0.0),
            }),
        );
    }

    /// ExoPlayer's states, as the Android plugin names them.
    fn state(&self) -> &'static str {
        let m = &self.mpv;
        if *self.loading.lock().unwrap() {
            "buffering"
        } else if m.flag("idle-active") {
            "idle"
        } else if m.flag("eof-reached") {
            "ended"
        } else if m.flag("paused-for-cache") || m.flag("seeking") {
            "buffering"
        } else if m.flag("pause") {
            "paused"
        } else if m.flag("core-idle") {
            "buffering"
        } else {
            "playing"
        }
    }

    fn finish_load(&self) {
        *self.loading.lock().unwrap() = false;
        self.loaded.notify_all();
    }

    /// Returns once the file is open and at `at`. A failure reaches the page as an `error`
    /// event, like ExoPlayer's.
    fn load(&self, url: &str, at: f64) -> Done {
        self.direct.store(!url.contains(".m3u8"), Ordering::Relaxed);
        *self.loading.lock().unwrap() = true;
        self.mpv.set("pause", "yes")?;
        if let Err(e) = self.mpv.command(&["loadfile", url, "replace"]) {
            self.finish_load();
            return Err(e);
        }
        let open = self.loading.lock().unwrap();
        let (mut open, _) = self
            .loaded
            .wait_timeout_while(open, LOAD_TIMEOUT, |l| *l)
            .unwrap();
        *open = false;
        drop(open);
        if at > 0.0 {
            let _ = self.mpv.command(&["seek", &at.to_string(), "absolute"]);
        }
        Ok(())
    }
}

/// Bytes read per window, counted only when the demuxer kept reading through all of it: the
/// cache stops reading when full, and an idle stretch would read as a slow link.
struct Reads {
    start: Instant,
    sum: f64,
    samples: u32,
    whole: bool,
}

impl Reads {
    fn new() -> Self {
        Reads {
            start: Instant::now(),
            sum: 0.0,
            samples: 0,
            whole: true,
        }
    }

    // ponytail: direct streams only. On HLS the demuxer reads segments as fast as the encoder
    // makes them, which says nothing about the link; ExoPlayer's per-segment meter has no mpv
    // counterpart here yet, so Auto on desktop HLS steps down on stalls but never back up.
    fn sample(&mut self, p: &Player) {
        if !p.direct.load(Ordering::Relaxed) || p.mpv.flag("demuxer-cache-idle") {
            self.whole = false;
        } else {
            self.sum += p.mpv.number("cache-speed").unwrap_or(0.0);
            self.samples += 1;
        }
        if self.start.elapsed() < READS_WINDOW {
            return;
        }
        if self.whole && self.samples > 0 && self.sum > 0.0 {
            let bits = self.sum / f64::from(self.samples) * 8.0;
            p.emit(
                "bandwidth",
                json!({ "bitrate": bits as u64, "source": "reads" }),
            );
        }
        *self = Reads::new();
    }
}

fn watch(p: Arc<Player>) {
    let mut last_state = "";
    let mut last_position = Instant::now();
    let mut reads = Reads::new();
    loop {
        match p.mpv.wait_event(TICK) {
            Event::Shutdown => return,
            Event::FileLoaded => p.finish_load(),
            Event::EndFile(Some(error)) => {
                p.finish_load();
                p.emit("error", json!({ "code": "mpv", "message": error }));
            }
            _ => {}
        }
        let state = p.state();
        if state != last_state {
            p.emit("state", json!({ "state": state }));
            if last_state == "playing" {
                p.emit_position();
            }
            last_state = state;
        }
        if state == "playing" && last_position.elapsed() >= POSITION_INTERVAL {
            p.emit_position();
            last_position = Instant::now();
        }
        reads.sample(&p);
    }
}

pub fn setup<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    let cache = app.path().app_cache_dir()?;
    let (fonts, subtitles) = (cache.join("fonts"), cache.join("subtitles"));
    std::fs::create_dir_all(&fonts)?;
    std::fs::create_dir_all(&subtitles)?;
    let fonts_dir = fonts.to_string_lossy().into_owned();
    let mpv = Mpv::new(&[
        ("vo", "libmpv"),
        ("hwdec", "auto-safe"),
        // ExoPlayer stays on its last frame at the end; the page shows its end screen over it.
        ("keep-open", "yes"),
        ("idle", "yes"),
        // Every control and subtitle choice belongs to the page.
        ("osd-level", "0"),
        ("osd-bar", "no"),
        ("input-default-bindings", "no"),
        ("input-vo-keyboard", "no"),
        ("sid", "no"),
        ("sub-auto", "no"),
        ("sub-fonts-dir", &fonts_dir),
        ("ytdl", "no"),
        ("terminal", "yes"),
        ("msg-level", "all=warn"),
    ])?;
    let player = Arc::new(Player {
        mpv: Arc::new(mpv),
        listeners: Mutex::default(),
        loading: Mutex::new(false),
        loaded: Condvar::new(),
        direct: AtomicBool::new(false),
        fonts,
        subtitles,
    });
    let watcher = player.clone();
    thread::Builder::new()
        .name("mpv-events".into())
        .spawn(move || watch(watcher))?;
    app.manage(player);
    Ok(())
}

pub fn attach<R: Runtime>(webview: Webview<R>) {
    static ATTACHED: AtomicBool = AtomicBool::new(false);
    let Some(player) = webview.try_state::<Arc<Player>>() else {
        return;
    };
    if ATTACHED.swap(true, Ordering::Relaxed) {
        return;
    }
    let mpv = player.mpv.clone();
    if let Err(e) = webview.with_webview(move |w| view::attach(w.inner(), mpv)) {
        eprintln!("no video layer: {e}");
    }
}

/// mpv decodes nearly everything and tone-maps HDR to the display itself, so it takes HDR as-is.
#[tauri::command]
pub fn capabilities() -> Value {
    // ponytail: a fixed 4K60 ceiling, not probed from VA-API or the monitors; mpv falls back to
    // software decoding past what the GPU does.
    let video = |codec: &str, profiles: &[&str]| {
        json!({ "codec": codec, "profiles": profiles, "max_level": null,
                "max_width": 3840, "max_height": 2160, "max_fps": 60 })
    };
    json!({
        "video": [
            video("h264", &["baseline", "constrained baseline", "main", "high", "high 10"]),
            video("hevc", &["main", "main 10"]),
            video("av1", &["main"]),
        ],
        "hdr": { "formats": ["hdr10", "hdr10plus", "hlg"], "dv_profiles": [] },
        "audio": {
            "codecs": ["aac", "ac3", "dts", "eac3", "flac", "mp3", "opus", "truehd", "vorbis"],
            "max_channels": 8,
            "passthrough": [],
        },
        "containers": ["mp4", "mkv"],
        "subtitles": ["webvtt", "srt", "ass", "pgs"],
        "hls_keyframe_exact": false,
    })
}

#[tauri::command]
pub async fn load(player: State<'_, Arc<Player>>, url: String, at: Option<f64>) -> Done {
    let p = player.inner().clone();
    tauri::async_runtime::spawn_blocking(move || p.load(&url, at.unwrap_or(0.0)))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn stop(player: State<'_, Arc<Player>>) -> Done {
    player.finish_load();
    player.mpv.command(&["stop"])
}

#[tauri::command]
pub fn play(player: State<'_, Arc<Player>>) -> Done {
    player.mpv.set("pause", "no")
}

#[tauri::command]
pub fn pause(player: State<'_, Arc<Player>>) -> Done {
    player.mpv.set("pause", "yes")
}

#[tauri::command]
pub fn seek(player: State<'_, Arc<Player>>, seconds: f64) -> Done {
    // Nothing to seek before a file opens; ExoPlayer ignores that too.
    let _ = player
        .mpv
        .command(&["seek", &seconds.max(0.0).to_string(), "absolute"]);
    player.emit_position();
    Ok(())
}

#[tauri::command]
pub fn speed(player: State<'_, Arc<Player>>, speed: f64) -> Done {
    player.mpv.set("speed", &speed.to_string())
}

#[tauri::command]
pub fn stats(player: State<'_, Arc<Player>>) -> Value {
    let m = &player.mpv;
    let version = m.get("mpv-version").unwrap_or_default();
    let dropped = m.number("frame-drop-count").unwrap_or(0.0)
        + m.number("decoder-frame-drop-count").unwrap_or(0.0);
    json!({
        "player": format!("{version} \u{00B7} Linux"),
        "version": version,
        "buffer": m.number("demuxer-cache-duration").unwrap_or(0.0),
        "videoBitrate": m.number("video-bitrate").unwrap_or(-1.0),
        "audioBitrate": m.number("audio-bitrate").unwrap_or(-1.0),
        "rendered": m.number("estimated-frame-number").unwrap_or(0.0),
        "dropped": dropped,
        "displayHz": m.number("display-fps-override").unwrap_or(0.0),
    })
}

#[tauri::command]
pub async fn subtitle(
    player: State<'_, Arc<Player>>,
    kind: Option<String>,
    url: Option<String>,
    fonts: Option<Vec<String>>,
) -> Done {
    let p = player.inner().clone();
    let _ = p.mpv.command(&["sub-remove"]);
    p.mpv.set("sid", "no")?;
    let Some(url) = url.filter(|_| matches!(kind.as_deref(), Some("ass" | "pgs"))) else {
        return Ok(());
    };
    // libass reads the fonts directory when a track opens, so they land before `sub-add`.
    for font in fonts.unwrap_or_default() {
        if let Err(e) = fetch_cached(&font, &p.fonts).await {
            eprintln!("subtitle font {font}: {e}");
        }
    }
    // A local file, as on Android: mpv opens a subtitle URL by seeking around in it, which a
    // server without range requests answers with a failure.
    let file = match fetch_cached(&url, &p.subtitles).await {
        Ok(file) => file,
        Err(e) => {
            eprintln!("subtitle {url}: {e}");
            p.emit("subtitleFailed", json!({}));
            return Ok(());
        }
    };
    // A track with no line in the file extracts to nothing, and mpv refuses an empty file.
    if std::fs::metadata(&file).is_ok_and(|m| m.len() == 0) {
        return Ok(());
    }
    let added = tauri::async_runtime::spawn_blocking(move || {
        let ok = p
            .mpv
            .command(&["sub-add", &file.to_string_lossy(), "select"]);
        (p, ok)
    })
    .await
    .map_err(|e| e.to_string())?;
    if let (p, Err(e)) = added {
        eprintln!("subtitle failed: {e}");
        p.emit("subtitleFailed", json!({}));
    }
    Ok(())
}

/// Downloads `url` into `dir` once, named by a hash of the URL and its last path segment.
async fn fetch_cached(url: &str, dir: &Path) -> Result<PathBuf, String> {
    let mut hash = DefaultHasher::new();
    url.hash(&mut hash);
    let last = url
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default();
    let file = dir.join(format!("{:x}-{last}", hash.finish()));
    if file.exists() {
        return Ok(file);
    }
    let bytes = reqwest::get(url)
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    let partial = file.with_extension("part");
    std::fs::write(&partial, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, &file).map_err(|e| e.to_string())?;
    Ok(file)
}

#[tauri::command]
pub fn timing(
    player: State<'_, Arc<Player>>,
    subtitle_offset: f64,
    subtitle_scale: f64,
    audio_delay: f64,
) -> Done {
    // Same signs as the Android plugin: positive shows subtitles and plays sound later.
    player.mpv.set("sub-delay", &subtitle_offset.to_string())?;
    player.mpv.set("sub-scale", &subtitle_scale.to_string())?;
    player.mpv.set("audio-delay", &audio_delay.to_string())
}

#[tauri::command]
pub fn register_listener(player: State<'_, Arc<Player>>, event: String, handler: Channel<Value>) {
    player
        .listeners
        .lock()
        .unwrap()
        .entry(event)
        .or_default()
        .push(handler);
}

#[tauri::command]
pub fn remove_listener(player: State<'_, Arc<Player>>, event: String, channel_id: u32) {
    if let Some(channels) = player.listeners.lock().unwrap().get_mut(&event) {
        channels.retain(|c| c.id() != channel_id);
    }
}
