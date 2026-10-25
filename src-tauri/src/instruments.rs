// Instrument presets (SFZ files) — see docs/instrument.md. Presets live
// anywhere under data/, but the preset browser only lists data/instruments/.
use std::path::Path;

use crate::app_root;

const PRESET_DIR: &str = "instruments";

fn reject_traversal(relative_path: &str) -> Result<&str, String> {
    let clean = relative_path.trim();
    if clean.contains("..") {
        return Err("Path traversal not allowed".to_string());
    }
    Ok(clean)
}

// Reads a data-relative text file (an .sfz preset). Raw string — parsing is
// done on the frontend (src/lib/instrument/sfz.ts).
#[tauri::command]
pub fn read_text_file(relative_path: String) -> Result<String, String> {
    let clean = reject_traversal(&relative_path)?;
    let path = app_root()?.join("data").join(clean);
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

fn collect_presets(dir: &Path, rel: &str, depth: u32, out: &mut Vec<String>) {
    if depth > 10 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(|e| e.ok()) {
        let Ok(name) = entry.file_name().into_string() else { continue };
        let child_rel = format!("{}/{}", rel, name);
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            collect_presets(&entry.path(), &child_rel, depth + 1, out);
        } else if name.to_lowercase().ends_with(".sfz") {
            out.push(child_rel);
        }
    }
}

// Every .sfz under data/instruments/ (recursive), as data-relative paths,
// sorted case-insensitively. The first folder level is the preset category.
#[tauri::command]
pub fn list_presets() -> Result<Vec<String>, String> {
    let dir = app_root()?.join("data").join(PRESET_DIR);
    let mut out = Vec::new();
    collect_presets(&dir, PRESET_DIR, 0, &mut out);
    out.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    Ok(out)
}
