// Downloadable instrument packs (see docs/instrument.md › Packs). The
// frontend picks the files of one instrument from its catalog
// (src/lib/instrument/packs/catalog.json) and this module downloads them into
// data/instruments/packs/ — one file at a time, with progress and a cancel
// flag. Only https://raw.githubusercontent.com/ URLs and destinations under
// instruments/packs/ are accepted.
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;

use crate::app_root;

const PACKS_DIR: &str = "instruments/packs/";
const URL_PREFIX: &str = "https://raw.githubusercontent.com/";

#[derive(Default)]
pub struct PackInstallState {
    cancel: Arc<AtomicBool>,
}

#[derive(Deserialize)]
pub struct PackFile {
    url: String,
    dest: String, // data-relative
    bytes: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    done_files: usize,
    total_files: usize,
    done_bytes: u64,
    total_bytes: u64,
}

// data-relative path → absolute, only inside data/instruments/packs/.
fn pack_path(dest: &str) -> Result<PathBuf, String> {
    let ok = dest.starts_with(PACKS_DIR)
        && !dest.contains("..")
        && !dest.contains('\\')
        && !dest.contains('\0')
        && !dest.starts_with('/');
    if !ok {
        return Err(format!("Refusing to write outside {}: {}", PACKS_DIR, dest));
    }
    Ok(app_root()?.join("data").join(dest))
}

fn download(agent: &ureq::Agent, file: &PackFile, cancel: &AtomicBool, mut on_bytes: impl FnMut(u64)) -> Result<(), String> {
    if !file.url.starts_with(URL_PREFIX) {
        return Err(format!("Refusing to download from {}", file.url));
    }
    let path = pack_path(&file.dest)?;
    // Already there at the expected size (an earlier, interrupted install).
    if std::fs::metadata(&path).map(|m| m.len() == file.bytes).unwrap_or(false) {
        on_bytes(file.bytes);
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let part = path.with_extension("part");
    let resp = agent.get(&file.url).call().map_err(|e| format!("{}: {}", file.url, e))?;
    let mut reader = resp.into_reader();
    let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(out);
            let _ = std::fs::remove_file(&part);
            return Err("cancelled".to_string());
        }
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        on_bytes(n as u64);
    }
    drop(out);
    std::fs::rename(&part, &path).map_err(|e| e.to_string())
}

// Downloads `files` in order, reporting progress after each chunk. Returns
// Err("cancelled") if cancel_pack_install() was called meanwhile.
#[tauri::command]
pub async fn install_pack_files(
    files: Vec<PackFile>,
    on_progress: Channel<InstallProgress>,
    state: State<'_, PackInstallState>,
) -> Result<(), String> {
    let cancel = state.cancel.clone();
    cancel.store(false, Ordering::Relaxed);
    tauri::async_runtime::spawn_blocking(move || {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(20))
            .timeout_read(Duration::from_secs(60))
            .build();
        let mut progress = InstallProgress {
            done_files: 0,
            total_files: files.len(),
            done_bytes: 0,
            total_bytes: files.iter().map(|f| f.bytes).sum(),
        };
        let mut last_sent = 0u64;
        for file in &files {
            download(&agent, file, &cancel, |n| {
                progress.done_bytes += n;
                // Throttle: one event per ~256 KB is plenty for a progress bar.
                if progress.done_bytes - last_sent >= 256 * 1024 {
                    last_sent = progress.done_bytes;
                    let _ = on_progress.send(progress.clone());
                }
            })?;
            progress.done_files += 1;
            let _ = on_progress.send(progress.clone());
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cancel_pack_install(state: State<'_, PackInstallState>) {
    state.cancel.store(true, Ordering::Relaxed);
}

// Deletes installed pack files (data-relative, under instruments/packs/),
// then prunes directories left empty.
#[tauri::command]
pub fn delete_pack_files(paths: Vec<String>) -> Result<(), String> {
    let root = app_root()?.join("data").join(PACKS_DIR);
    for p in &paths {
        let path = pack_path(p)?;
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        let mut dir = path.parent().map(|d| d.to_path_buf());
        while let Some(d) = dir {
            if d == root || !d.starts_with(&root) || std::fs::remove_dir(&d).is_err() {
                break; // not empty (or the packs root itself)
            }
            dir = d.parent().map(|x| x.to_path_buf());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_accepts_paths_inside_packs() {
        assert!(pack_path("instruments/packs/vsco2/Keys/a.wav").is_ok());
        assert!(pack_path("instruments/packs/../../preferences.xml").is_err());
        assert!(pack_path("samples/x.wav").is_err());
        assert!(pack_path("/etc/passwd").is_err());
        assert!(pack_path("instruments/packs/a\\..\\b").is_err());
    }
}
