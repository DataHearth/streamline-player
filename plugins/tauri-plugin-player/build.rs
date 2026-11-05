// Commands are implemented in Kotlin; Tauri forwards any command the Rust side
// does not handle to the registered mobile plugin. These names only feed the ACL.
const COMMANDS: &[&str] = &[
    "capabilities",
    "load",
    "play",
    "pause",
    "seek",
    "speed",
    "stats",
    "subtitle",
    "timing",
    "registerListener",
    "removeListener",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
