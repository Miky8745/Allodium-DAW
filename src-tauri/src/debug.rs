// Dev-only profiling harness, driven by env vars (see CLAUDE.md › Commands ›
// Profiling):
//   ALLODIUM_PROFILE=1            frontend profiler on; lines go to stderr
//   ALLODIUM_AUTOPLAY=<project>   open it and play it automatically
//   ALLODIUM_AUTOPLAY_SECONDS=N   how long each autoplay phase runs (default 20)
//   ALLODIUM_AUTOPLAY_PANELS=…    windows to open first: `all`, or a comma list of
//                                    channelrack,pianoroll,mixer,overview,noplaylist
// The frontend (src/lib/debug/) does all the measuring; this module only
// hands it the config and prints what it sends back.
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugConfig {
    profile: bool,
    autoplay: String,
    autoplay_seconds: f64,
    panels: String,
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).map_or(false, |v| !v.is_empty() && v != "0")
}

#[tauri::command]
pub fn debug_config() -> DebugConfig {
    DebugConfig {
        profile: env_flag("ALLODIUM_PROFILE"),
        autoplay: std::env::var("ALLODIUM_AUTOPLAY").unwrap_or_default(),
        autoplay_seconds: std::env::var("ALLODIUM_AUTOPLAY_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20.0),
        panels: std::env::var("ALLODIUM_AUTOPLAY_PANELS").unwrap_or_default(),
    }
}

#[tauri::command]
pub fn perf_log(lines: Vec<String>) {
    for line in lines {
        eprintln!("[perf] {line}");
    }
}

// Lets an autoplay run quit the app when it's done.
#[tauri::command]
pub fn debug_exit(app: tauri::AppHandle) {
    app.exit(0);
}
