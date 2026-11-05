use serde_json::{Value, json};

// ponytail: fixed dev server (dev/mock-streamline.py); device pairing replaces it.
// 10.0.2.2 is the Android emulator's alias for the host.
const SERVER: &str = match option_env!("STREAMLINE_URL") {
    Some(url) => url,
    None if cfg!(target_os = "android") => "http://10.0.2.2:8080",
    None => "http://127.0.0.1:8080",
};

struct Api(reqwest::Client);

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
async fn artwork(client: reqwest::Client, path: String) -> tauri::http::Response<Vec<u8>> {
    let fetched = async {
        let resp = client
            .get(format!("{SERVER}/api/v1/posters{path}"))
            .send()
            .await?
            .error_for_status()?;
        resp.bytes().await
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
    async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let mut req = self.0.request(method, format!("{SERVER}/api/v1{path}"));
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        let value = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).map_err(|e| e.to_string())?
        };
        if status.is_success() {
            Ok(value)
        } else {
            Err(value.to_string())
        }
    }
}

#[tauri::command]
fn server_host() -> &'static str {
    SERVER.split_once("://").map_or(SERVER, |(_, host)| host)
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
async fn playback_status(api: tauri::State<'_, Api>) -> Result<Value, String> {
    api.call(reqwest::Method::GET, "/playback/status", None)
        .await
}

#[tauri::command]
async fn start_session(
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
    api.call(reqwest::Method::POST, "/playback/sessions", Some(body))
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
    let resp = api.0.get(url).send().await.map_err(|e| e.to_string())?;
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
async fn stop_session(api: tauri::State<'_, Api>, id: String) -> Result<(), String> {
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
    let client = reqwest::Client::new();
    let images = client.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_player::init())
        .manage(Api(client))
        .register_asynchronous_uri_scheme_protocol("artwork", move |_ctx, request, responder| {
            let client = images.clone();
            let path = request.uri().path().to_owned();
            tauri::async_runtime::spawn(async move {
                responder.respond(artwork(client, path).await);
            });
        })
        .invoke_handler(tauri::generate_handler![
            server_host,
            library,
            title,
            watch_state,
            set_watched,
            playback_status,
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
