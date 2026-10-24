mod capture;
mod debug;
mod midi;
mod preferences;
mod project;
use capture::{system_audio_start, system_audio_stop};
use debug::{debug_config, debug_exit, perf_log};
use midi::{midi_list_ports, midi_schedule, midi_send, midi_set_inputs, midi_set_outputs, midi_stop_all};
use preferences::{load_preferences, save_preferences};
use project::{load_history, load_project, save_history, save_project};
use tauri::Manager;

pub(crate) fn app_root() -> Result<std::path::PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    Ok(if cwd.file_name().and_then(|n| n.to_str()) == Some("src-tauri") {
        cwd.parent().unwrap_or(&cwd).to_path_buf()
    } else {
        cwd
    })
}

pub(crate) fn projects_root() -> Result<std::path::PathBuf, String> {
    Ok(app_root()?.join("data").join("projects"))
}

#[derive(serde::Serialize)]
struct FileNode {
    name: String,
    is_dir: bool,
    children: Vec<FileNode>,
}

fn read_dir_tree(dir: &std::path::Path, depth: usize) -> Result<Vec<FileNode>, String> {
    if depth > 10 {
        return Ok(vec![]);
    }
    let mut entries: Vec<FileNode> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let is_dir = e.file_type().ok()?.is_dir();
            let children = if is_dir {
                read_dir_tree(&e.path(), depth + 1).unwrap_or_default()
            } else {
                vec![]
            };
            Some(FileNode { name, is_dir, children })
        })
        .collect();
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

#[tauri::command]
fn list_project_files(name: String) -> Result<Vec<FileNode>, String> {
    let path = projects_root()?.join(name.trim());
    if !path.exists() {
        return Ok(vec![]);
    }
    read_dir_tree(&path, 0)
}

#[tauri::command]
fn list_data_files() -> Result<Vec<FileNode>, String> {
    let path = app_root()?.join("data");
    if !path.exists() {
        return Ok(vec![]);
    }
    read_dir_tree(&path, 0)
}

#[tauri::command]
fn list_projects() -> Result<Vec<String>, String> {
    let dir = projects_root()?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                entry.file_name().into_string().ok()
            } else {
                None
            }
        })
        .collect();
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    Ok(names)
}

#[tauri::command]
fn get_data_root() -> Result<String, String> {
    let path = app_root()?.join("data");
    path.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Non-UTF8 path".to_string())
}

// Some sample libraries ship files as a RIFF/WAVE shell whose "fmt " chunk
// declares format tag 0x674F (WAVE_FORMAT_OGG_VORBIS_MODE_1, "Og") — the
// "data" chunk in that case isn't PCM at all, it's a complete, valid Ogg
// Vorbis stream written verbatim. Browsers' decodeAudioData doesn't recognize
// 0x674F, so such files silently fail to decode; but stripping the outer
// WAVE shell down to just the data chunk's payload yields a plain .ogg file
// any Vorbis decoder (incl. the browser's) opens normally.
fn unwrap_ogg_in_wav(bytes: Vec<u8>) -> Vec<u8> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return bytes;
    }
    let mut pos: usize = 12;
    let mut format_tag: u16 = 0;
    let mut data_chunk: Option<(usize, usize)> = None;
    while pos + 8 <= bytes.len() {
        let chunk_id = &bytes[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let payload_start = pos + 8;
        let payload_end = payload_start.saturating_add(chunk_size).min(bytes.len());
        if chunk_id == b"fmt " && payload_end - payload_start >= 2 {
            format_tag = u16::from_le_bytes([bytes[payload_start], bytes[payload_start + 1]]);
        } else if chunk_id == b"data" {
            data_chunk = Some((payload_start, payload_end - payload_start));
        }
        pos = payload_start.saturating_add(chunk_size).saturating_add(chunk_size % 2);
    }
    if format_tag == 0x674F {
        if let Some((start, len)) = data_chunk {
            return bytes[start..start + len].to_vec();
        }
    }
    bytes
}

// Returned as a raw binary IPC response (an ArrayBuffer on the frontend),
// not Vec<u8> — Tauri would JSON-encode a Vec<u8> as a number array
// ("[82,73,70,70,...]"), ~4x the file size, parsed on the webview's main
// thread, which froze the UI for every sample loaded (worst when a
// multisample folder loads dozens at once).
#[tauri::command]
fn read_audio_bytes(relative_path: String) -> Result<tauri::ipc::Response, String> {
    let clean = relative_path.trim();
    if clean.contains("..") {
        return Err("Path traversal not allowed".to_string());
    }
    let path = app_root()?.join("data").join(clean);
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(unwrap_ogg_in_wav(bytes)))
}

const AUDIO_EXTENSIONS: [&str; 4] = ["wav", "ogg", "mp3", "flac"];

// Lists the immediate audio-file children of a data-relative folder (not
// recursive) — used when a folder is dropped onto a channel to build a
// multisample instrument from its contents.
#[tauri::command]
fn list_dir_files(relative_path: String) -> Result<Vec<String>, String> {
    let clean = relative_path.trim();
    if clean.contains("..") {
        return Err("Path traversal not allowed".to_string());
    }
    let dir = app_root()?.join("data").join(clean);
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            if !e.file_type().ok()?.is_file() {
                return None;
            }
            let name = e.file_name().into_string().ok()?;
            let ext = name.rsplit('.').next()?.to_lowercase();
            AUDIO_EXTENSIONS.contains(&ext.as_str()).then_some(name)
        })
        .collect();
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    Ok(names)
}

#[tauri::command]
fn create_project(name: String) -> Result<(), String> {
    let clean = name.trim();
    if clean.is_empty() {
        return Err("Project name cannot be empty".to_string());
    }
    if clean.contains('/') || clean.contains('\\') || clean.contains("..") {
        return Err("Project name contains invalid characters".to_string());
    }
    let project_path = projects_root()?.join(clean);
    if project_path.exists() {
        return Err(format!("A project named '{}' already exists", clean));
    }
    std::fs::create_dir_all(&project_path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
// `ALLODIUM_BACKGROUND=1 npm run tauri dev` starts the window minimized
// so a dev launch (e.g. by Claude Code) doesn't pop over whatever the user
// is doing. The frontend asks via this command so openProject() can stay
// minimized after its maximize().
fn background_launch_enabled() -> bool {
    std::env::var("ALLODIUM_BACKGROUND").map_or(false, |v| !v.is_empty() && v != "0")
}

#[tauri::command]
fn background_launch() -> bool {
    background_launch_enabled()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(midi::new_state())
        .manage(capture::CaptureState::default())
        .setup(|app| {
            let state = app.state::<midi::SharedMidi>().inner().clone();
            midi::start_threads(app.handle().clone(), state);
            if background_launch_enabled() {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.minimize();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            background_launch,
            debug_config,
            perf_log,
            debug_exit,
            list_projects,
            list_project_files,
            list_data_files,
            create_project,
            get_data_root,
            read_audio_bytes,
            list_dir_files,
            save_project,
            load_project,
            save_history,
            load_history,
            load_preferences,
            save_preferences,
            midi_list_ports,
            midi_set_inputs,
            midi_set_outputs,
            midi_send,
            midi_schedule,
            midi_stop_all,
            system_audio_start,
            system_audio_stop
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
