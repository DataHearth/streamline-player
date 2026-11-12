use crate::Api;
use serde_json::{Value, json};
use std::{
    fs,
    hash::{BuildHasher, Hasher},
    path::PathBuf,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};

const KEY_NAME: &str = "device-key";
const HEALTH_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(target_os = "android")]
pub const KEYSTORE: &str = "Android Keystore";
#[cfg(target_os = "linux")]
pub const KEYSTORE: &str = "Secret Service";

/// Where the server address and this install's id live: plain app data, unlike the key.
fn data_file(app: &AppHandle, name: &str) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join(name))
}

pub fn read_server(app: &AppHandle) -> Option<String> {
    data_file(app, "server")
        .and_then(|f| fs::read_to_string(f).ok())
        .filter(|s| !s.is_empty())
}

fn write_data(app: &AppHandle, name: &str, value: &str) {
    if let Some(file) = data_file(app, name) {
        let _ = file.parent().map(fs::create_dir_all);
        let _ = fs::write(file, value);
    }
}

/// Stable per install: approving a pairing with a known `client_id` replaces that device's old key.
fn client_id(app: &AppHandle) -> String {
    if let Some(id) = data_file(app, "client-id").and_then(|f| fs::read_to_string(f).ok()) {
        return id;
    }
    // RandomState is seeded from the OS's randomness: two draws make 128 random bits.
    let draw = || {
        std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish()
    };
    let id = format!("{:016x}{:016x}", draw(), draw());
    write_data(app, "client-id", &id);
    id
}

#[cfg(target_os = "linux")]
fn entry() -> Result<keyring_core::Entry, String> {
    static STORE: std::sync::Once = std::sync::Once::new();
    STORE.call_once(|| {
        if let Ok(store) = zbus_secret_service_keyring_store::Store::new() {
            keyring_core::set_default_store(store);
        }
    });
    keyring_core::Entry::new("streamline-player", KEY_NAME).map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
fn vault(app: &AppHandle, command: &str, value: Option<&str>) -> Result<Value, String> {
    app.state::<tauri_plugin_player::Android<tauri::Wry>>()
        .0
        .run_mobile_plugin::<Value>(command, json!({ "name": KEY_NAME, "value": value }))
        .map_err(|e| e.to_string())
}

// Only Android reaches its key store through the app.
#[cfg_attr(not(target_os = "android"), allow(unused_variables))]
pub fn load_key(app: &AppHandle) -> Result<Option<String>, String> {
    #[cfg(target_os = "linux")]
    return match entry()?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    };
    #[cfg(target_os = "android")]
    return Ok(vault(app, "vaultGet", None)?["value"]
        .as_str()
        .map(str::to_owned));
}

#[cfg_attr(not(target_os = "android"), allow(unused_variables))]
fn store_key(app: &AppHandle, key: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return entry()?.set_password(key).map_err(|e| e.to_string());
    #[cfg(target_os = "android")]
    return vault(app, "vaultSet", Some(key)).map(drop);
}

#[cfg_attr(not(target_os = "android"), allow(unused_variables))]
pub fn delete_key(app: &AppHandle) {
    #[cfg(target_os = "linux")]
    let _ = entry().and_then(|e| e.delete_credential().map_err(|e| e.to_string()));
    #[cfg(target_os = "android")]
    let _ = vault(app, "vaultDelete", None);
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .expect("blocking task panicked")
}

/// Typed as "streamline.home.arpa", meaning https; an explicit http:// is kept for servers
/// on the LAN without TLS.
fn normalize(typed: &str) -> String {
    let t = typed.trim().trim_end_matches('/');
    if t.contains("://") {
        t.to_owned()
    } else {
        format!("https://{t}")
    }
}

#[tauri::command]
pub async fn auth_state(api: tauri::State<'_, Api>) -> Result<Value, String> {
    let c = api.conn().await;
    Ok(json!({
        "server": c.server,
        "paired": c.key.is_some(),
        "keystore": KEYSTORE,
        "store_error": c.store_error,
    }))
}

/// Checks the address answers as Streamline before keeping it. Returns the round trip in ms,
/// or an error code the page words: `unreachable`, `not_streamline`.
#[tauri::command]
pub async fn connect(
    app: AppHandle,
    api: tauri::State<'_, Api>,
    url: String,
) -> Result<u64, String> {
    let url = normalize(&url);
    let started = Instant::now();
    let resp = api
        .http
        .get(format!("{url}/health"))
        .timeout(HEALTH_TIMEOUT)
        .send()
        .await
        .map_err(|_| "unreachable")?;
    if !resp.status().is_success() {
        return Err("not_streamline".into());
    }
    let ms = started.elapsed().as_millis() as u64;
    if api.conn().await.server.as_deref() != Some(url.as_str()) {
        write_data(&app, "server", &url);
        api.forget_key().await;
        api.set_server(url);
    }
    Ok(ms)
}

fn device(app: &AppHandle) -> (String, &'static str) {
    #[cfg(target_os = "android")]
    {
        let info = app
            .state::<tauri_plugin_player::Android<tauri::Wry>>()
            .0
            .run_mobile_plugin::<Value>("deviceInfo", json!({}))
            .unwrap_or_default();
        let tv = info["platform"] == "Android TV";
        let name = info["name"].as_str().unwrap_or("Android TV").to_owned();
        (name, if tv { "android-tv" } else { "android" })
    }
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        let name =
            fs::read_to_string("/etc/hostname").map_or("Linux".to_owned(), |h| h.trim().to_owned());
        (name, "linux")
    }
}

fn qr_svg(data: &str) -> Result<String, String> {
    use qrcode::render::svg;
    let code = qrcode::QrCode::new(data).map_err(|e| e.to_string())?;
    Ok(code
        .render::<svg::Color>()
        .quiet_zone(false)
        .dark_color(svg::Color("#0b0b10"))
        .light_color(svg::Color("transparent"))
        .build())
}

/// Starts a pairing; the device code stays in Rust, the page only gets what it shows.
#[tauri::command]
pub async fn pair_start(app: AppHandle, api: tauri::State<'_, Api>) -> Result<Value, String> {
    let (name, platform) = {
        let app = app.clone();
        blocking(move || device(&app)).await
    };
    let started = api
        .call(
            reqwest::Method::POST,
            "/auth/device/code",
            Some(json!({ "client_id": client_id(&app), "name": name, "platform": platform })),
        )
        .await?;
    let device_code = started["device_code"]
        .as_str()
        .ok_or("no device_code")?
        .to_owned();
    *api.pairing.lock().unwrap() = Some(device_code);
    let complete = started["verification_uri_complete"]
        .as_str()
        .unwrap_or_default();
    Ok(json!({
        "code": started["user_code"],
        "name": name,
        "uri": started["verification_uri"],
        "uri_complete": complete,
        "expires_in": started["expires_in"],
        "interval": started["interval"],
        "qr": qr_svg(complete)?,
    }))
}

/// Opens a web page in the system browser; anything but http(s) is refused.
#[tauri::command]
pub async fn open_external(url: String) -> Result<(), String> {
    let parsed = reqwest::Url::parse(&url).map_err(|e| e.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!("not a web address: {url}"));
    }
    blocking(move || {
        tauri_plugin_opener::open_url(parsed.as_str(), None::<&str>).map_err(|e| e.to_string())
    })
    .await
}

/// One poll: "pending", "slow_down", "denied", "expired" or "paired".
#[tauri::command]
pub async fn pair_poll(app: AppHandle, api: tauri::State<'_, Api>) -> Result<&'static str, String> {
    let Some(device_code) = api.pairing.lock().unwrap().clone() else {
        return Ok("expired");
    };
    let answer = api
        .call(
            reqwest::Method::POST,
            "/auth/device/token",
            Some(json!({ "device_code": device_code })),
        )
        .await;
    let key = match answer {
        Ok(v) => v["api_key"].as_str().ok_or("no api_key")?.to_owned(),
        Err(e) => {
            let code = serde_json::from_str::<Value>(&e)
                .ok()
                .and_then(|v| v["error"]["code"].as_str().map(str::to_owned));
            return match code.as_deref() {
                Some("authorization_pending") => Ok("pending"),
                Some("slow_down") => Ok("slow_down"),
                Some("access_denied") => Ok("denied"),
                Some("expired_token") => Ok("expired"),
                _ => Err(e),
            };
        }
    };
    *api.pairing.lock().unwrap() = None;
    let stored = {
        let (app, key) = (app.clone(), key.clone());
        blocking(move || store_key(&app, &key)).await
    };
    api.set_key(key, stored.err());
    Ok("paired")
}

/// Drops this device's key on the server (best effort) and here.
#[tauri::command]
pub async fn sign_out(api: tauri::State<'_, Api>) -> Result<(), String> {
    let _ = api
        .call(reqwest::Method::DELETE, "/devices/current", None)
        .await;
    api.forget_key().await;
    Ok(())
}
