use serde_json::{Value, json};

// ponytail: fixed dev server (dev/mock-streamline.py); device pairing replaces it.
// 10.0.2.2 is the Android emulator's alias for the host.
const SERVER: &str = match option_env!("STREAMLINE_URL") {
    Some(url) => url,
    None if cfg!(target_os = "android") => "http://10.0.2.2:8080",
    None => "http://127.0.0.1:8080",
};

struct Api(reqwest::Client);

impl Api {
    async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let mut req = self
            .0
            .request(method, format!("{SERVER}/api/v1/playback{path}"));
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
async fn playback_status(api: tauri::State<'_, Api>) -> Result<Value, String> {
    api.call(reqwest::Method::GET, "/status", None).await
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
    api.call(reqwest::Method::POST, "/sessions", Some(body))
        .await
}

#[tauri::command]
async fn media_tracks(api: tauri::State<'_, Api>, media_id: String) -> Result<Value, String> {
    api.call(
        reqwest::Method::GET,
        &format!("/media/{media_id}/tracks"),
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
        &format!("/sessions/{id}"),
        Some(changes),
    )
    .await
}

#[tauri::command]
async fn stop_session(api: tauri::State<'_, Api>, id: String) -> Result<(), String> {
    api.call(reqwest::Method::DELETE, &format!("/sessions/{id}"), None)
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
        &format!("/sessions/{id}/progress"),
        Some(body),
    )
    .await
    .map(drop)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_player::init())
        .manage(Api(reqwest::Client::new()))
        .invoke_handler(tauri::generate_handler![
            playback_status,
            media_tracks,
            subtitle_text,
            start_session,
            patch_session,
            stop_session,
            heartbeat
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
