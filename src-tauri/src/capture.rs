// System-audio capture for the Toolbar's audio monitor (BlackBox) — records
// the default sink's PulseAudio/PipeWire *monitor* source (everything the
// system is playing, this app included) and streams it to the frontend as
// raw mono f32 chunks over a binary IPC Channel (never a JSON number array,
// same lesson as read_audio_bytes). Visualization only — nothing here is
// ever played back, so the app's own output being in the mix can't feed back.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;

use libpulse_binding::def::BufferAttr;
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;
use tauri::ipc::{Channel, InvokeResponseBody};

const CHANNELS: u8 = 2;
// ~10ms per read at 48kHz: small enough that the scope tracks what you hear.
const CHUNK_FRAMES: usize = 480;

struct Capture {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

#[derive(Default)]
pub struct CaptureState(Mutex<Option<Capture>>);

fn stop_capture(state: &CaptureState) {
    let taken = state.0.lock().unwrap().take();
    if let Some(c) = taken {
        c.stop.store(true, Ordering::Relaxed);
        let _ = c.thread.join();
    }
}

fn open_monitor(rate: u32) -> Result<Simple, String> {
    let spec = Spec { format: Format::FLOAT32NE, channels: CHANNELS, rate };
    if !spec.is_valid() {
        return Err(format!("invalid capture sample rate {rate}"));
    }
    let frag = (CHUNK_FRAMES * CHANNELS as usize * 4) as u32;
    let attr = BufferAttr { maxlength: u32::MAX, tlength: u32::MAX, prebuf: u32::MAX, minreq: u32::MAX, fragsize: frag };
    Simple::new(None, "Allodium", Direction::Record, Some("@DEFAULT_MONITOR@"), "Audio monitor", &spec, None, Some(&attr))
        .map_err(|e| format!("couldn't open system audio monitor: {e}"))
}

/// Starts (or restarts) capture at `sample_rate` — the frontend passes its
/// AudioContext's rate so PulseAudio does any resampling. Each message on
/// `on_data` is CHUNK_FRAMES little-endian f32 mono samples.
#[tauri::command]
pub fn system_audio_start(
    state: tauri::State<'_, CaptureState>,
    sample_rate: u32,
    on_data: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    stop_capture(&state);
    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = stop.clone();
    // The Simple handle isn't Send, so it's opened on the capture thread;
    // the open result is reported back before this command returns.
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let thread = std::thread::spawn(move || {
        let pa = match open_monitor(sample_rate) {
            Ok(pa) => { let _ = ready_tx.send(Ok(())); pa }
            Err(e) => { let _ = ready_tx.send(Err(e)); return; }
        };
        let mut raw = vec![0u8; CHUNK_FRAMES * CHANNELS as usize * 4];
        let mut mono = Vec::with_capacity(CHUNK_FRAMES * 4);
        while !stop_t.load(Ordering::Relaxed) {
            if pa.read(&mut raw).is_err() { break; }
            mono.clear();
            for frame in raw.chunks_exact(CHANNELS as usize * 4) {
                let l = f32::from_ne_bytes(frame[0..4].try_into().unwrap());
                let r = f32::from_ne_bytes(frame[4..8].try_into().unwrap());
                mono.extend_from_slice(&((l + r) * 0.5).to_le_bytes());
            }
            // Frontend gone (webview reload) → stop quietly.
            if on_data.send(InvokeResponseBody::Raw(mono.clone())).is_err() { break; }
        }
    });
    match ready_rx.recv() {
        Ok(Ok(())) => {
            *state.0.lock().unwrap() = Some(Capture { stop, thread });
            Ok(())
        }
        Ok(Err(e)) => { let _ = thread.join(); Err(e) }
        Err(_) => Err("capture thread exited unexpectedly".into()),
    }
}

#[tauri::command]
pub fn system_audio_stop(state: tauri::State<'_, CaptureState>) {
    stop_capture(&state);
}

#[cfg(test)]
mod tests {
    // Needs a running PulseAudio/PipeWire server: `cargo test --lib -- --ignored capture`.
    #[test]
    #[ignore]
    fn reads_from_default_monitor() {
        let pa = super::open_monitor(48000).expect("open monitor");
        let mut raw = vec![0u8; super::CHUNK_FRAMES * 8];
        pa.read(&mut raw).expect("read");
    }
}
