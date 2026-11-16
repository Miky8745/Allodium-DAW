//! Real system-audio-loopback capture for the toolbar's "System audio"
//! monitor source (`preferences::MonitorSource::System`) — a genuine
//! implementation now, not the "no capture exists" placeholder it
//! shipped as.
//!
//! `cpal` (this app's only audio dependency) has no PipeWire or PulseAudio
//! host on Linux, only ALSA, and plain ALSA has no portable device name
//! for "whatever the default output sink is currently playing" — that's
//! a PipeWire/Pulse-level concept (a sink's "monitor"), not an ALSA one.
//! Binding PipeWire's native API directly is a much larger dependency
//! than one toolbar meter justifies, so this instead shells out to
//! `pw-record`, which ships with PipeWire itself and already exists on
//! this machine. Its raw stdout feeds the exact same `MeterState` the
//! DAW's own mix already uses — same peak decay, same scope ring buffer,
//! just a second instance.
//!
//! Getting a loopback (as opposed to microphone) capture out of
//! `pw-record` needs one property most examples online skip:
//! `stream.capture.sink=true`. Without it, `--target <sink>` alone is
//! silently ignored — the session manager still auto-links a plain
//! `media.class=Stream/Input/Audio` stream to the default *source* (the
//! mic), target or no target. Caught by an actual user report ("you're
//! capturing the microphone"), then confirmed directly with `pw-link -l`
//! while a `pw-record` invocation missing the property was running: it
//! showed `pw-record:input_FL/FR` linked from
//! `alsa_input....:capture_FL/FR`. Adding `stream.capture.sink=true` to
//! the `-P` properties and re-checking the same way showed the expected
//! link instead, from `alsa_output....:monitor_FL/FR` — the sink's own
//! loopback monitor ports.
//!
//! Requires PipeWire's command-line tools on `PATH`. On a system without
//! them (pure PulseAudio, pure ALSA, a different OS entirely), starting
//! capture fails with a reported error (surfaced through
//! `AudioEngine::system_capture_error`) rather than silently doing
//! nothing.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::thread::JoinHandle;

use super::MeterState;

const RATE: u32 = 44_100;
const CHANNELS: usize = 2;

/// Finds PipeWire's current default output sink by name, by parsing
/// `pw-metadata`'s one-shot dump of the session's metadata — confirmed
/// interactively that it prints the current state and exits on its own
/// (it is not a long-running monitor that would need to be killed).
fn default_sink_name() -> Result<String, String> {
    let output = Command::new("pw-metadata").output().map_err(|e| format!("couldn't run `pw-metadata` ({e}) — is PipeWire installed?"))?;
    if !output.status.success() {
        return Err("`pw-metadata` exited with an error".to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if !line.contains("key:'default.audio.sink'") {
            continue;
        }
        if let Some(start) = line.find("\"name\":\"") {
            let rest = &line[start + 8..];
            if let Some(end) = rest.find('"') {
                return Ok(rest[..end].to_string());
            }
        }
    }
    Err("no default audio sink reported by PipeWire".to_string())
}

/// Owns the `pw-record` child process and its reader thread for exactly
/// as long as "System audio" is selected. Dropping this (on switching
/// back to "DAW audio", or on app exit) kills the process; its reader
/// thread then exits on its own next read (the closed pipe reads as EOF).
pub struct SystemCapture {
    child: Child,
    reader: Option<JoinHandle<()>>,
}

impl SystemCapture {
    /// Spawns `pw-record` targeting the default sink's monitor and starts
    /// a background thread decoding its raw interleaved `f32` stdout into
    /// `meter`.
    pub fn start(meter: Arc<MeterState>) -> Result<Self, String> {
        let sink = default_sink_name()?;
        let mut child = Command::new("pw-record")
            .args([
                "--target",
                &sink,
                // `media.class=Stream/Input/Audio` alone isn't enough: the
                // session manager still auto-links a plain Input/Audio
                // stream to the default *source* (the microphone),
                // ignoring `--target` entirely — confirmed interactively
                // via `pw-link -l` while this was running, which showed
                // `pw-record:input_FL/FR` connected to
                // `alsa_input....:capture_FL/FR`, not the sink at all.
                // `stream.capture.sink=true` is the documented PipeWire
                // property that tells it "link this to the *target's
                // monitor* ports" instead — re-checked the same way after
                // adding it, which showed the expected
                // `alsa_output....:monitor_FL/FR` link.
                "-P",
                "{ media.class=Stream/Input/Audio stream.capture.sink=true }",
                "--format",
                "f32",
                "--rate",
                &RATE.to_string(),
                "--channels",
                &CHANNELS.to_string(),
                "-a",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("couldn't run `pw-record` ({e}) — is PipeWire installed?"))?;

        let mut stdout = child.stdout.take().expect("pw-record spawned with piped stdout");
        let reader = std::thread::spawn(move || {
            let frame_bytes = 4 * CHANNELS;
            let mut buf = [0u8; 4096];
            let mut carry: Vec<u8> = Vec::new();
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) => break, // EOF — process exited or was killed.
                    Ok(n) => {
                        carry.extend_from_slice(&buf[..n]);
                        let usable = carry.len() - (carry.len() % frame_bytes);
                        for frame in carry[..usable].chunks_exact(frame_bytes) {
                            let l = f32::from_le_bytes(frame[0..4].try_into().unwrap());
                            let r = f32::from_le_bytes(frame[4..8].try_into().unwrap());
                            meter.push_frame(l, r);
                        }
                        carry.drain(..usable);
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self { child, reader: Some(reader) })
    }
}

impl Drop for SystemCapture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
    }
}

