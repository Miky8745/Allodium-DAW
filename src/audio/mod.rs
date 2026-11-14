//! The audio engine: `cpal` device I/O, a sample-accurate scheduler, and
//! voice mixing. See `docs/stage-03-audio-engine.md` for the design
//! decisions (why scheduling lives inside the callback itself rather than
//! the original's separate look-ahead timer, why playback takes an
//! immutable snapshot instead of hot-editing shared state, etc).

mod envelope;
mod pitch;
mod sample;

pub use envelope::Envelope;
pub use pitch::{pitch_to_playback_rate, step_duration_secs};
pub use sample::{SampleBuffer, decode_sample};

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::project::{Note, ProjectFile};

/// One channel's audio-thread-ready playback data for a single pattern,
/// flattened out of `ProjectFile`'s pattern-scoped content so the audio
/// callback never has to walk the project model's structure itself.
pub struct ChannelVoiceData {
    pub sample: Arc<SampleBuffer>,
    pub steps: Vec<bool>,
    pub notes: Vec<Note>,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
}

/// Everything one playback session needs, built once from the project
/// model right before `play()`. Deliberately immutable for the session's
/// lifetime — there's no UI that edits a channel while it's playing yet
/// (that's a Channel Rack/Piano Roll concern), so there's nothing to gain
/// from the complexity of a lock-free hot-swap path before it's needed.
/// Revisit when a later stage actually needs live edits mid-playback.
pub struct PlaybackSnapshot {
    pub channels: Vec<ChannelVoiceData>,
    pub pattern_length_steps: u32,
}

/// Builds a `PlaybackSnapshot` for the project's currently selected
/// pattern. A channel with no sample loaded in `samples` is skipped
/// (silent) rather than erroring — matches the original's "no sample
/// assigned yet" behavior.
pub fn build_snapshot(
    project: &ProjectFile,
    samples: &HashMap<String, Arc<SampleBuffer>>,
) -> Result<PlaybackSnapshot, String> {
    let pattern = project
        .patterns
        .items
        .iter()
        .find(|p| p.id == project.selected_pattern_id)
        .ok_or_else(|| "selected pattern not found".to_string())?;
    let pattern_length_steps = (pattern.length_beats * 4.0).round().max(1.0) as u32;

    let channels = project
        .channels
        .items
        .iter()
        .filter_map(|ch| {
            let sample = samples.get(&ch.sample_path)?.clone();
            let content = project
                .content
                .items
                .iter()
                .find(|e| e.pattern_id == pattern.id && e.channel_id == ch.id);
            Some(ChannelVoiceData {
                sample,
                steps: content.map(|c| c.steps.clone()).unwrap_or_default(),
                notes: content.map(|c| c.notes.clone()).unwrap_or_default(),
                volume: ch.volume as f32,
                pan: ch.pan as f32,
                muted: ch.muted,
            })
        })
        .collect();

    Ok(PlaybackSnapshot { channels, pattern_length_steps })
}

/// A currently-sounding one-shot: a sample, an envelope, and a playback
/// position advancing at `playback_rate` (pitch shift combined with any
/// sample-rate/device-rate mismatch) frames per output frame.
struct Voice {
    sample: Arc<SampleBuffer>,
    pos: f64,
    playback_rate: f64,
    volume: f32,
    /// `0.0` = full left, `0.5` = center, `1.0` = full right (matches the
    /// project model's own pan convention, default `0.5`).
    pan: f32,
    envelope: Envelope,
    frames_played: u64,
}

impl Voice {
    fn new(sample: Arc<SampleBuffer>, pitch_rate: f64, volume: f32, pan: f32, envelope: Envelope, out_sample_rate: u32) -> Self {
        let rate_correction = sample.sample_rate as f64 / out_sample_rate as f64;
        Voice { sample, pos: 0.0, playback_rate: pitch_rate * rate_correction, volume, pan, envelope, frames_played: 0 }
    }

    /// Renders one output frame into `mix_l`/`mix_r`, advances position,
    /// and reports whether the voice is still alive.
    fn render(&mut self, mix_l: &mut f32, mix_r: &mut f32) -> bool {
        let Some(env_gain) = self.envelope.gain_at(self.frames_played) else { return false };
        let total_frames = self.sample.frame_count();
        let idx = self.pos.floor() as u64;
        if idx >= total_frames {
            return false;
        }
        let frac = self.pos.fract() as f32;
        let s0 = self.read_mono(idx);
        let s1 = if idx + 1 < total_frames { self.read_mono(idx + 1) } else { s0 };
        let sample_val = s0 + (s1 - s0) * frac;

        let gain = env_gain * self.volume;
        // Equal-power pan, matching the project model's 0..1 pan range.
        let angle = self.pan.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
        *mix_l += sample_val * gain * angle.cos();
        *mix_r += sample_val * gain * angle.sin();

        self.pos += self.playback_rate;
        self.frames_played += 1;
        true
    }

    fn read_mono(&self, idx: u64) -> f32 {
        let ch = self.sample.channels.max(1) as u64;
        let base = (idx * ch) as usize;
        if self.sample.channels <= 1 {
            self.sample.data[base]
        } else {
            (self.sample.data[base] + self.sample.data[base + 1]) * 0.5
        }
    }
}

struct PlaybackState {
    playing: AtomicBool,
    bpm_centibeats: AtomicU32,
    samples_played: AtomicU64,
    sample_rate: AtomicU32,
    pattern_length_steps: AtomicU32,
}

/// Owns the live `cpal` output stream plus the cross-thread playback
/// state the UI thread can read/write without ever taking a lock the
/// audio thread (real-time, must never block) could also be holding.
pub struct AudioEngine {
    stream: Option<cpal::Stream>,
    state: Arc<PlaybackState>,
}

impl AudioEngine {
    pub fn new() -> Self {
        Self {
            stream: None,
            state: Arc::new(PlaybackState {
                playing: AtomicBool::new(false),
                bpm_centibeats: AtomicU32::new(12000),
                samples_played: AtomicU64::new(0),
                sample_rate: AtomicU32::new(48000),
                pattern_length_steps: AtomicU32::new(16),
            }),
        }
    }

    pub fn is_playing(&self) -> bool {
        self.state.playing.load(Ordering::Relaxed)
    }

    pub fn set_bpm(&self, bpm: f64) {
        self.state.bpm_centibeats.store((bpm * 100.0).round() as u32, Ordering::Relaxed);
    }

    /// Continuous (non-floored) step position within the loop, suitable
    /// for driving a smooth playhead once one exists.
    pub fn current_step(&self) -> f64 {
        let sr = self.state.sample_rate.load(Ordering::Relaxed).max(1) as f64;
        let bpm = self.state.bpm_centibeats.load(Ordering::Relaxed) as f64 / 100.0;
        let samples = self.state.samples_played.load(Ordering::Relaxed) as f64;
        let len = self.state.pattern_length_steps.load(Ordering::Relaxed).max(1) as f64;
        let steps_per_sec = bpm / 60.0 * 4.0;
        (samples / sr * steps_per_sec).rem_euclid(len)
    }

    /// Builds a fresh output stream from the given snapshot and starts
    /// playing immediately. Scheduling happens inside the callback itself
    /// against its own running sample count, not a separate look-ahead
    /// timer — a deliberate departure from the original's two-clock
    /// design, since a native real-time callback already *is* the clock
    /// and can decide exactly when a step begins with zero drift.
    pub fn play(&mut self, snapshot: PlaybackSnapshot, bpm: f64) -> Result<(), String> {
        self.stop();

        let host = cpal::default_host();
        let device = host.default_output_device().ok_or_else(|| "no default audio output device".to_string())?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        let out_sample_rate = config.sample_rate().0;
        let out_channels = config.channels() as usize;

        self.state.bpm_centibeats.store((bpm * 100.0).round() as u32, Ordering::Relaxed);
        self.state.sample_rate.store(out_sample_rate, Ordering::Relaxed);
        self.state.pattern_length_steps.store(snapshot.pattern_length_steps.max(1), Ordering::Relaxed);
        self.state.samples_played.store(0, Ordering::Relaxed);
        self.state.playing.store(true, Ordering::Relaxed);

        let state = self.state.clone();
        let pattern_length_steps = snapshot.pattern_length_steps.max(1) as i64;
        let mut voices: Vec<Voice> = Vec::new();
        let mut last_triggered_step: i64 = -1;

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    for frame in data.chunks_mut(out_channels) {
                        if !state.playing.load(Ordering::Relaxed) {
                            frame.iter_mut().for_each(|s| *s = 0.0);
                            continue;
                        }

                        let samples_played = state.samples_played.fetch_add(1, Ordering::Relaxed);
                        let t = samples_played as f64 / out_sample_rate as f64;
                        let bpm = state.bpm_centibeats.load(Ordering::Relaxed) as f64 / 100.0;
                        let steps_per_sec = bpm / 60.0 * 4.0;
                        let whole_step = (t * steps_per_sec).floor() as i64;
                        let pattern_step = whole_step.rem_euclid(pattern_length_steps);

                        if whole_step != last_triggered_step {
                            last_triggered_step = whole_step;
                            for ch in &snapshot.channels {
                                if ch.muted {
                                    continue;
                                }
                                if ch.notes.is_empty() {
                                    if ch.steps.get(pattern_step as usize).copied().unwrap_or(false) {
                                        voices.push(Voice::new(
                                            ch.sample.clone(),
                                            1.0,
                                            ch.volume,
                                            ch.pan,
                                            Envelope::ring(out_sample_rate),
                                            out_sample_rate,
                                        ));
                                    }
                                } else {
                                    for note in &ch.notes {
                                        if note.start.round() as i64 == pattern_step {
                                            let rate = pitch_to_playback_rate(note.pitch);
                                            let duration_secs = note.length * step_duration_secs(bpm);
                                            let duration_frames = (duration_secs * out_sample_rate as f64).max(1.0) as u64;
                                            voices.push(Voice::new(
                                                ch.sample.clone(),
                                                rate,
                                                ch.volume * note.velocity as f32,
                                                ch.pan,
                                                Envelope::shaped(duration_frames, out_sample_rate),
                                                out_sample_rate,
                                            ));
                                        }
                                    }
                                }
                            }
                        }

                        let mut mix_l = 0.0f32;
                        let mut mix_r = 0.0f32;
                        voices.retain_mut(|v| v.render(&mut mix_l, &mut mix_r));
                        // Soft clip rather than a hard clamp, so a loud
                        // mix compresses gracefully instead of crackling.
                        mix_l = mix_l.tanh();
                        mix_r = mix_r.tanh();

                        match frame.len() {
                            1 => frame[0] = (mix_l + mix_r) * 0.5,
                            _ => {
                                for (i, s) in frame.iter_mut().enumerate() {
                                    *s = if i % 2 == 0 { mix_l } else { mix_r };
                                }
                            }
                        }
                    }
                },
                |err| eprintln!("[audio] stream error: {err}"),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        self.stream = Some(stream);
        Ok(())
    }

    /// Cheap by design: no device stream is torn down (`self.stream` is
    /// just dropped and rebuilt on the next `play()`, see
    /// `docs/stage-03-audio-engine.md` for the measured cost of that).
    pub fn stop(&mut self) {
        self.state.playing.store(false, Ordering::Relaxed);
        self.stream = None;
    }
}
