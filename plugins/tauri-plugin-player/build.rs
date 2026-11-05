// Commands are implemented in Kotlin; Tauri forwards any command the Rust side
// does not handle to the registered mobile plugin. These names only feed the ACL.
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
    "registerListener",
    "removeListener",
    "remove_listener",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
