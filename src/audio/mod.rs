//! The audio engine: `cpal` device I/O, a sample-accurate scheduler, and
//! voice mixing. See `docs/stage-03-audio-engine.md` and
//! `docs/stage-04-channel-rack.md` for the design decisions (single-clock
//! scheduling inside the callback, the message-passing hot-swap path that
//! replaced Stage 3's immutable-snapshot simplification once Channel Rack
//! actually needed live edits mid-playback).

mod envelope;
mod pitch;
mod sample;

pub use envelope::Envelope;
pub use pitch::{pitch_to_playback_rate, step_duration_secs};
pub use sample::{SampleBuffer, decode_sample};

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;

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
    /// Already folded in: an explicit mute, or (any channel soloed and
    /// this one isn't) — see `build_snapshot`'s "isAudible" logic, ported
    /// from the original's same-named rule ("explicit mute always wins
    /// regardless of solo").
    pub muted: bool,
}

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

    let any_soloed = project.channels.items.iter().any(|c| c.soloed);

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
                muted: ch.muted || (any_soloed && !ch.soloed),
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

/// Sent from the UI thread into the running audio callback. Applied at
/// the top of each buffer (not per-frame — up to one buffer's worth of
/// latency, a few milliseconds, which is imperceptible for a UI edit).
enum EngineMessage {
    /// Replaces the content the scheduler plays from — pushed on every
    /// step/volume/pan/mute/solo/channel-list edit so changes take effect
    /// immediately without interrupting playback position.
    UpdateContent { channels: Vec<ChannelVoiceData>, pattern_length_steps: u32 },
    /// A one-shot outside the scheduler, for click-to-audition. Plays
    /// regardless of transport state.
    Preview { sample: Arc<SampleBuffer>, volume: f32, pan: f32 },
}

/// Owns a *persistent* `cpal` output stream (built once, alive for the
/// engine's whole life) plus the cross-thread playback state the UI
/// thread can read/write without ever taking a lock the audio thread
/// (real-time, must never block) could also be holding. Kept alive even
/// while stopped so click-to-audition previews work regardless of
/// transport state, and so play/stop never pays a device-reopen cost —
/// cheaper than Stage 3's original rebuild-per-play design, which this
/// replaced once Channel Rack needed previews to work while stopped too.
pub struct AudioEngine {
    _stream: cpal::Stream,
    sender: mpsc::Sender<EngineMessage>,
    state: Arc<PlaybackState>,
}

impl AudioEngine {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or_else(|| "no default audio output device".to_string())?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        let out_sample_rate = config.sample_rate().0;
        let out_channels = config.channels() as usize;

        let state = Arc::new(PlaybackState {
            playing: AtomicBool::new(false),
            bpm_centibeats: AtomicU32::new(12000),
            samples_played: AtomicU64::new(0),
            sample_rate: AtomicU32::new(out_sample_rate),
            pattern_length_steps: AtomicU32::new(16),
        });

        let (sender, receiver) = mpsc::channel::<EngineMessage>();
        let callback_state = state.clone();
        let mut channels: Vec<ChannelVoiceData> = Vec::new();
        let mut pattern_length_steps: i64 = 16;
        let mut voices: Vec<Voice> = Vec::new();
        let mut last_triggered_step: i64 = -1;

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    for msg in receiver.try_iter() {
                        match msg {
                            EngineMessage::UpdateContent { channels: new_channels, pattern_length_steps: new_len } => {
                                channels = new_channels;
                                pattern_length_steps = new_len.max(1) as i64;
                            }
                            EngineMessage::Preview { sample, volume, pan } => {
                                voices.push(Voice::new(sample, 1.0, volume, pan, Envelope::ring(out_sample_rate), out_sample_rate));
                            }
                        }
                    }

                    for frame in data.chunks_mut(out_channels) {
                        if callback_state.playing.load(Ordering::Relaxed) {
                            let samples_played = callback_state.samples_played.fetch_add(1, Ordering::Relaxed);
                            let t = samples_played as f64 / out_sample_rate as f64;
                            let bpm = callback_state.bpm_centibeats.load(Ordering::Relaxed) as f64 / 100.0;
                            let steps_per_sec = bpm / 60.0 * 4.0;
                            let whole_step = (t * steps_per_sec).floor() as i64;
                            let pattern_step = whole_step.rem_euclid(pattern_length_steps);

                            if whole_step != last_triggered_step {
                                last_triggered_step = whole_step;
                                for ch in &channels {
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
                        }

                        // Voices (scheduled or previewed) keep rendering
                        // regardless of transport state, so a preview
                        // click plays out even while stopped.
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

        Ok(Self { _stream: stream, sender, state })
    }

    pub fn is_playing(&self) -> bool {
        self.state.playing.load(Ordering::Relaxed)
    }

    pub fn set_bpm(&self, bpm: f64) {
        self.state.bpm_centibeats.store((bpm * 100.0).round() as u32, Ordering::Relaxed);
    }

    /// Continuous (non-floored) step position within the loop, suitable
    /// for driving a smooth playhead (Channel Rack's step highlight).
    pub fn current_step(&self) -> f64 {
        let sr = self.state.sample_rate.load(Ordering::Relaxed).max(1) as f64;
        let bpm = self.state.bpm_centibeats.load(Ordering::Relaxed) as f64 / 100.0;
        let samples = self.state.samples_played.load(Ordering::Relaxed) as f64;
        let len = self.state.pattern_length_steps.load(Ordering::Relaxed).max(1) as f64;
        let steps_per_sec = bpm / 60.0 * 4.0;
        (samples / sr * steps_per_sec).rem_euclid(len)
    }

    /// Pushes new content for the scheduler to play from — call on every
    /// edit (step toggle, volume/pan, mute/solo, add/remove channel).
    /// Does not touch transport state, so playback position continues
    /// uninterrupted while content changes under it.
    pub fn set_content(&self, snapshot: PlaybackSnapshot) {
        self.state.pattern_length_steps.store(snapshot.pattern_length_steps.max(1), Ordering::Relaxed);
        let _ = self.sender.send(EngineMessage::UpdateContent {
            channels: snapshot.channels,
            pattern_length_steps: snapshot.pattern_length_steps,
        });
    }

    pub fn play(&self, bpm: f64) {
        self.state.bpm_centibeats.store((bpm * 100.0).round() as u32, Ordering::Relaxed);
        self.state.samples_played.store(0, Ordering::Relaxed);
        self.state.playing.store(true, Ordering::Relaxed);
    }

    /// Cheap by design: no device stream is torn down, just an atomic
    /// flip — the stream itself lives for the engine's whole lifetime.
    pub fn stop(&self) {
        self.state.playing.store(false, Ordering::Relaxed);
    }

    /// A one-shot outside the scheduler, for click-to-audition. Plays
    /// regardless of whether the transport is running.
    pub fn preview(&self, sample: Arc<SampleBuffer>, volume: f32, pan: f32) {
        let _ = self.sender.send(EngineMessage::Preview { sample, volume, pan });
    }
}
