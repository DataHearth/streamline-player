use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};

#[cfg(target_os = "linux")]
mod desktop;
#[cfg(target_os = "linux")]
mod mpv;
#[cfg(target_os = "linux")]
mod view;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    let builder = Builder::new("player").setup(|_app, _api| {
        #[cfg(target_os = "android")]
        _api.register_android_plugin("com.datahearth.player", "PlayerPlugin")?;
        #[cfg(target_os = "linux")]
        if let Err(e) = desktop::setup(_app) {
            // The rest of the app still works; playback fails with the page's error screen.
            eprintln!("no player: {e}");
        }
        Ok(())
    });
    // Only on desktop: Tauri hands a command the Rust side handles to Rust, even on Android,
    // where the Kotlin plugin must answer instead.
    #[cfg(target_os = "linux")]
    let builder = builder
        .invoke_handler(tauri::generate_handler![
            desktop::capabilities,
            desktop::load,
            desktop::stop,
            desktop::play,
            desktop::pause,
            desktop::seek,
            desktop::speed,
            desktop::stats,
            desktop::subtitle,
            desktop::timing,
            desktop::register_listener,
            desktop::remove_listener,
        ])
        .on_webview_ready(desktop::attach);
    builder.build()
}
