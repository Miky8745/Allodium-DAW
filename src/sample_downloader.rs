//! "Download Default Samples" (the File Browser's Samples-tab button) —
//! fetches a small basic drum/bass kit from the LMMS project's own bundled
//! sample library, not from anything resembling a commercial DAW's
//! proprietary content. LMMS (<https://github.com/LMMS/lmms>) is free
//! software distributed under the GNU General Public License v2, and its
//! repository's single top-level `LICENSE.txt` covers the whole tree as
//! distributed there, samples included — downloading these files from
//! LMMS's own public GitHub repo and keeping that license text alongside
//! them (see `LICENSE_NOTE` below) is a legitimate, legally clean way to
//! seed this app with content to drum-program against, unlike extracting
//! another DAW's bundled library.
//!
//! Runs on a plain background `std::thread` (the same shape
//! `audio::system_capture` already uses for its own child-process I/O),
//! reporting progress through an `Arc<Mutex<DownloadState>>` the File
//! Browser polls once per render while a download is in flight — no
//! GPUI executor/`Task` involved, since this has nothing to hand back to
//! the render loop except plain data.

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const BASE_URL: &str = "https://raw.githubusercontent.com/LMMS/lmms/master/data/samples";

/// (filename written into the destination folder, path under LMMS's own
/// `data/samples/`) — a small, genuinely basic kit: four-on-the-floor
/// drums plus one bass hit, enough to program a first beat with.
const DEFAULT_SAMPLES: &[(&str, &str)] = &[
    ("Kick.ogg", "drums/kick01.ogg"),
    ("Snare.ogg", "drums/snare01.ogg"),
    ("Clap.ogg", "drums/clap01.ogg"),
    ("Hihat Closed.ogg", "drums/hihat_closed01.ogg"),
    ("Hihat Open.ogg", "drums/hihat_opened01.ogg"),
    ("Crash.ogg", "drums/crash01.ogg"),
    ("Ride.ogg", "drums/ride01.ogg"),
    ("Tom.ogg", "drums/tom_mid01.ogg"),
    ("Rim.ogg", "drums/rim01.ogg"),
    ("Wood.ogg", "drums/wood01.ogg"),
    ("Bass.ogg", "basses/bass01.ogg"),
];

const LICENSE_NOTE: &str = "\
These sample files were downloaded from the LMMS project's own bundled\n\
sample library:\n\
\n\
    https://github.com/LMMS/lmms/tree/master/data/samples\n\
\n\
LMMS is free software distributed under the GNU General Public License,\n\
version 2 (GPL-2.0)data/samples -- see https://github.com/LMMS/lmms/blob/master/LICENSE.txt\n\
for the full text, which (per that repository's own top-level LICENSE.txt)\n\
covers everything in the repository as distributed there, this sample\n\
library included. Keep this notice alongside the files if you redistribute\n\
them.\n";

#[derive(Clone)]
pub enum DownloadState {
    Running { done: usize, total: usize },
    Done { count: usize },
    Failed(String),
}

pub struct SampleDownloader {
    state: Arc<Mutex<DownloadState>>,
}

impl SampleDownloader {
    /// Spawns the download and returns immediately; `state()` reflects
    /// progress from then on. `dest_dir` is created if it doesn't exist
    /// (expected to be `<samples tab root>/Default Samples`).
    pub fn start(dest_dir: PathBuf) -> Self {
        let state = Arc::new(Mutex::new(DownloadState::Running { done: 0, total: DEFAULT_SAMPLES.len() }));
        let state_bg = state.clone();
        thread::spawn(move || {
            if let Err(e) = fs::create_dir_all(&dest_dir) {
                *state_bg.lock().unwrap() = DownloadState::Failed(format!("couldn't create '{}': {e}", dest_dir.display()));
                return;
            }
            let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).build();
            let mut succeeded = 0usize;
            let mut last_err: Option<String> = None;
            for (i, (name, suffix)) in DEFAULT_SAMPLES.iter().enumerate() {
                let url = format!("{BASE_URL}/{suffix}");
                match fetch(&agent, &url) {
                    Ok(bytes) => match fs::write(dest_dir.join(name), &bytes) {
                        Ok(()) => succeeded += 1,
                        Err(e) => last_err = Some(format!("couldn't write '{name}': {e}")),
                    },
                    Err(e) => last_err = Some(format!("couldn't download '{name}': {e}")),
                }
                *state_bg.lock().unwrap() = DownloadState::Running { done: i + 1, total: DEFAULT_SAMPLES.len() };
            }
            if succeeded > 0 {
                let _ = fs::write(dest_dir.join("LICENSE.txt"), LICENSE_NOTE);
            }
            *state_bg.lock().unwrap() = if succeeded > 0 {
                DownloadState::Done { count: succeeded }
            } else {
                DownloadState::Failed(last_err.unwrap_or_else(|| "no samples downloaded".to_string()))
            };
        });
        Self { state }
    }

    pub fn state(&self) -> DownloadState {
        self.state.lock().unwrap().clone()
    }
}

fn fetch(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    let response = agent.get(url).call().map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    response.into_reader().read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}
