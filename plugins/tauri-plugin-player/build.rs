// Android answers these in Kotlin (Tauri forwards commands the Rust side does not handle to the
// mobile plugin), Linux in src/desktop.rs. These names only feed the ACL.
const COMMANDS: &[&str] = &[
    "capabilities",
    "load",
    "play",
    "pause",
    "seek",
    "stop",
    "speed",
    "stats",
    "subtitle",
    "timing",
    "voice",
    "registerListener",
    "register_listener",
    "removeListener",
    "remove_listener",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
