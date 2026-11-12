use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, RwLock},
};
use tauri::{AppHandle, Emitter, Manager};

mod auth;

#[derive(Clone, Default)]
struct Conn {
    server: Option<String>,
    key: Option<String>,
    /// Why the key could not go into secure storage: it then lasts until the app closes.
    store_error: Option<String>,
    loaded: bool,
}

enum Body {
    None,
    Json(Value),
    Raw(Vec<u8>, String),
}

pub struct Api {
    http: reqwest::Client,
    app: AppHandle,
    conn: RwLock<Conn>,
    /// The device code of the pairing in progress: a credential, so it never reaches the page.
    pairing: Mutex<Option<String>>,
}

fn host_of(url: &str) -> &str {
    url.split_once("://").map_or(url, |(_, host)| host)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Movies,
    Series,
}

impl Kind {
    fn path(&self) -> &'static str {
        match self {
            Kind::Movies => "/movies",
            Kind::Series => "/series",
        }
    }
}

// Images load through <img>, which can't carry the device key, so they pass through here.
async fn artwork(app: AppHandle, path: String) -> tauri::http::Response<Vec<u8>> {
    let api = app.state::<Api>();
    let conn = api.conn().await;
    let fetched = async {
        let mut req = api.http.get(format!(
            "{}/api/v1/posters{path}",
            conn.server.unwrap_or_default()
        ));
        if let Some(key) = &conn.key {
            req = req.header("X-API-Key", key);
        }
        req.send().await?.error_for_status()?.bytes().await
    };
    match fetched.await {
        Ok(bytes) => tauri::http::Response::builder()
            .header("Content-Type", "image/jpeg")
            .body(bytes.to_vec()),
        Err(_) => tauri::http::Response::builder()
            .status(404)
            .body(Vec::new()),
    }
    .expect("static response parts are valid")
}

impl Api {
    /// Reads the server address and the key on first use: the key store can block, and on
    /// Android it answers on the main thread, so never during setup.
    async fn conn(&self) -> Conn {
        let conn = self.conn.read().unwrap().clone();
        if conn.loaded {
            return conn;
        }
        let app = self.app.clone();
        let (server, key) = tauri::async_runtime::spawn_blocking(move || {
            (auth::read_server(&app), auth::load_key(&app))
        })
        .await
        .expect("key store read panicked");
        let mut c = self.conn.write().unwrap();
        if !c.loaded {
            *c = Conn {
                server,
                key: key.unwrap_or_else(|e| {
                    eprintln!("device key unreadable: {e}");
                    None
                }),
                store_error: None,
                loaded: true,
            };
        }
        c.clone()
    }

    fn set_server(&self, url: String) {
        self.conn.write().unwrap().server = Some(url);
    }

    fn set_key(&self, key: String, store_error: Option<String>) {
        let mut c = self.conn.write().unwrap();
        c.key = Some(key);
        c.store_error = store_error;
    }

    async fn forget_key(&self) {
        self.conn.write().unwrap().key = None;
        let app = self.app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || auth::delete_key(&app)).await;
    }

    async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        self.call_with(method, path, body.map_or(Body::None, Body::Json))
            .await
    }

    async fn call_with(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Body,
    ) -> Result<Value, String> {
        let conn = self.conn().await;
        let Some(server) = conn.server else {
            return Err(json!({ "error": { "code": "no_server" } }).to_string());
        };
        let mut req = self.http.request(method, format!("{server}/api/v1{path}"));
        if let Some(key) = &conn.key {
            req = req.header("X-API-Key", key);
        }
        req = match body {
            Body::None => req,
            Body::Json(b) => req.json(&b),
            Body::Raw(bytes, content_type) => req.header("Content-Type", content_type).body(bytes),
        };
        let resp = req.send().await.map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        let value = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).map_err(|e| e.to_string())?
        };
        // The key was revoked from the account page, or a password change dropped every key.
        if status == reqwest::StatusCode::UNAUTHORIZED && conn.key.is_some() {
            self.forget_key().await;
            let _ = self.app.emit("signed-out", ());
        }
        if status.is_success() {
            Ok(value)
        } else {
            Err(value.to_string())
        }
    }
}

#[tauri::command]
async fn server_host(api: tauri::State<'_, Api>) -> Result<String, String> {
    Ok(api
        .conn()
        .await
        .server
        .as_deref()
        .map(host_of)
        .unwrap_or_default()
        .to_owned())
}

#[tauri::command]
async fn library(api: tauri::State<'_, Api>, kind: Kind) -> Result<Value, String> {
    api.call(reqwest::Method::GET, kind.path(), None).await
}

#[tauri::command]
async fn title(api: tauri::State<'_, Api>, kind: Kind, id: u32) -> Result<Value, String> {
    api.call(reqwest::Method::GET, &format!("{}/{id}", kind.path()), None)
        .await
}

#[tauri::command]
async fn person(api: tauri::State<'_, Api>, id: String) -> Result<Value, String> {
    // A slug from the server, but it lands in a URL path: refuse anything that could leave it.
    if !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(json!({ "error": { "code": "not_found" } }).to_string());
    }
    api.call(reqwest::Method::GET, &format!("/people/{id}"), None)
        .await
}

#[tauri::command]
async fn watch_state(api: tauri::State<'_, Api>) -> Result<Value, String> {
    api.call(reqwest::Method::GET, "/watchstate", None).await
}

#[tauri::command]
async fn set_watched(
    api: tauri::State<'_, Api>,
    media_id: String,
    watched: bool,
) -> Result<Value, String> {
    api.call(
        reqwest::Method::PUT,
        &format!("/watchstate/{media_id}"),
        Some(json!({ "watched": watched })),
    )
    .await
}

#[tauri::command]
async fn me(api: tauri::State<'_, Api>) -> Result<Value, String> {
    api.call(reqwest::Method::GET, "/me", None).await
}

#[tauri::command]
async fn home(api: tauri::State<'_, Api>) -> Result<Value, String> {
    api.call(reqwest::Method::GET, "/home", None).await
}

#[tauri::command]
async fn update_me(
    api: tauri::State<'_, Api>,
    name: Option<String>,
    email: Option<String>,
) -> Result<Value, String> {
    let mut body = serde_json::Map::new();
    if let Some(name) = name {
        body.insert("display_name".into(), name.into());
    }
    if let Some(email) = email {
        body.insert("device_email".into(), email.into());
    }
    api.call(reqwest::Method::PATCH, "/me", Some(body.into()))
        .await
}

/// The image travels as the raw body of the call, its type in `x-picture-type`.
#[tauri::command]
async fn set_picture(
    api: tauri::State<'_, Api>,
    request: tauri::ipc::Request<'_>,
) -> Result<(), String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(json!({ "error": { "code": "invalid_request" } }).to_string());
    };
    let content_type = request
        .headers()
        .get("x-picture-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    api.call_with(
        reqwest::Method::PUT,
        "/me/picture",
        Body::Raw(bytes.clone(), content_type),
    )
    .await
    .map(drop)
}

#[tauri::command]
async fn remove_picture(api: tauri::State<'_, Api>) -> Result<(), String> {
    api.call(reqwest::Method::DELETE, "/me/picture", None)
        .await
        .map(drop)
}

#[tauri::command]
async fn revoke_device(api: tauri::State<'_, Api>, id: String) -> Result<(), String> {
    if !id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(json!({ "error": { "code": "not_found" } }).to_string());
    }
    api.call(reqwest::Method::DELETE, &format!("/devices/{id}"), None)
        .await
        .map(drop)
}

#[tauri::command]
async fn devices(api: tauri::State<'_, Api>) -> Result<Value, String> {
    api.call(reqwest::Method::GET, "/devices", None).await
}

#[tauri::command]
async fn playback_status(api: tauri::State<'_, Api>) -> Result<Value, String> {
    api.call(reqwest::Method::GET, "/playback/status", None)
        .await
}

/// The session this device has open, on disk so that a launch after a crash, or a page reload,
/// can stop it: both skip `stop_session`, and the dead session would hold its conversion slot
/// until Streamline's heartbeat timeout.
fn open_session_file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("open-session"))
}

fn record_open_session(app: &AppHandle, id: &str) {
    if let Some(file) = open_session_file(app) {
        let _ = file.parent().map(fs::create_dir_all);
        let _ = fs::write(file, id);
    }
}

fn forget_open_session(app: &AppHandle, id: &str) {
    if let Some(file) = open_session_file(app)
        && fs::read_to_string(&file).is_ok_and(|open| open == id)
    {
        let _ = fs::remove_file(file);
    }
}

fn stop_left_over_session(app: &AppHandle) {
    let Some(file) = open_session_file(app) else {
        return;
    };
    let Ok(id) = fs::read_to_string(&file) else {
        return;
    };
    // Removed before the DELETE is sent, so a session started meanwhile is never the one forgotten.
    let _ = fs::remove_file(file);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let path = format!("/playback/sessions/{id}");
        let _ = app
            .state::<Api>()
            .call(reqwest::Method::DELETE, &path, None)
            .await;
    });
}

#[tauri::command]
async fn start_session(
    app: AppHandle,
    api: tauri::State<'_, Api>,
    media_id: String,
    capabilities: Value,
    position: f64,
    tracks: Value,
) -> Result<Value, String> {
    let mut body = json!({
        "media_id": media_id,
        "capabilities": capabilities,
        "position": position,
    });
    if let (Some(body), Some(tracks)) = (body.as_object_mut(), tracks.as_object()) {
        body.extend(tracks.clone());
    }
    let session = api
        .call(reqwest::Method::POST, "/playback/sessions", Some(body))
        .await?;
    if let Some(id) = session["session_id"].as_str() {
        record_open_session(&app, id);
    }
    Ok(session)
}

#[tauri::command]
async fn decisions(
    api: tauri::State<'_, Api>,
    media_id: String,
    capabilities: Value,
    options: Value,
) -> Result<Value, String> {
    api.call(
        reqwest::Method::POST,
        "/playback/decisions",
        Some(json!({
            "media_id": media_id,
            "capabilities": capabilities,
            "options": options,
        })),
    )
    .await
}

#[tauri::command]
async fn media_tracks(api: tauri::State<'_, Api>, media_id: String) -> Result<Value, String> {
    api.call(
        reqwest::Method::GET,
        &format!("/playback/media/{media_id}/tracks"),
        None,
    )
    .await
}

#[tauri::command]
async fn subtitle_text(api: tauri::State<'_, Api>, url: String) -> Result<String, String> {
    let resp = api.http.get(url).send().await.map_err(|e| e.to_string())?;
    resp.error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn patch_session(
    api: tauri::State<'_, Api>,
    id: String,
    changes: Value,
) -> Result<Value, String> {
    api.call(
        reqwest::Method::PATCH,
        &format!("/playback/sessions/{id}"),
        Some(changes),
    )
    .await
}

#[tauri::command]
async fn session_info(api: tauri::State<'_, Api>, id: String) -> Result<Value, String> {
    api.call(
        reqwest::Method::GET,
        &format!("/playback/sessions/{id}"),
        None,
    )
    .await
}

#[tauri::command]
async fn stop_session(
    app: AppHandle,
    api: tauri::State<'_, Api>,
    id: String,
) -> Result<(), String> {
    forget_open_session(&app, &id);
    api.call(
        reqwest::Method::DELETE,
        &format!("/playback/sessions/{id}"),
        None,
    )
    .await
    .map(drop)
}

#[tauri::command]
async fn heartbeat(
    api: tauri::State<'_, Api>,
    id: String,
    position: f64,
    paused: bool,
) -> Result<(), String> {
    let body = json!({ "position": position, "paused": paused });
    api.call(
        reqwest::Method::POST,
        &format!("/playback/sessions/{id}/progress"),
        Some(body),
    )
    .await
    .map(drop)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_player::init())
        .setup(|app| {
            app.manage(Api {
                http: reqwest::Client::new(),
                app: app.handle().clone(),
                conn: RwLock::default(),
                pairing: Mutex::default(),
            });
            #[cfg(target_os = "linux")]
            // WebKitGTK (seen on 2.54.1) zooms pages by gtk-xft-dpi / 96, and GTK reports -1
            // when no XSETTINGS daemon runs (Hyprland, sway): pages get a zoom of -1/96, a
            // negative innerWidth and devicePixelRatio, a collapsed layout and clicks that
            // miss. Drop this once WebKitGTK ignores an unset DPI.
            if let Some(settings) = gtk::Settings::default() {
                use gtk::prelude::GtkSettingsExt;
                if settings.gtk_xft_dpi() <= 0 {
                    settings.set_gtk_xft_dpi(96 * 1024);
                }
            }
            Ok(())
        })
        // On each load, not only at launch: a reload drops the page without `stop_session`
        // while this process, and the file naming the session, live on.
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                stop_left_over_session(webview.app_handle());
            }
        })
        .register_asynchronous_uri_scheme_protocol("artwork", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = request.uri().path().to_owned();
            tauri::async_runtime::spawn(async move {
                responder.respond(artwork(app, path).await);
            });
        })
        .invoke_handler(tauri::generate_handler![
            auth::auth_state,
            auth::connect,
            auth::pair_start,
            auth::pair_poll,
            auth::open_external,
            auth::sign_out,
            server_host,
            library,
            title,
            person,
            watch_state,
            set_watched,
            me,
            home,
            update_me,
            set_picture,
            remove_picture,
            revoke_device,
            devices,
            playback_status,
            decisions,
            media_tracks,
            subtitle_text,
            start_session,
            patch_session,
            session_info,
            stop_session,
            heartbeat
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
