//! The audio engine: `cpal` device I/O, a sample-accurate scheduler, and
//! voice mixing. See `docs/stage-03-audio-engine.md` and
//! `docs/stage-04-channel-rack.md` for the design decisions (single-clock
//! scheduling inside the callback, the message-passing hot-swap path that
//! replaced Stage 3's immutable-snapshot simplification once Channel Rack
//! actually needed live edits mid-playback).

mod envelope;
mod pitch;
mod sample;
mod system_capture;

pub use envelope::Envelope;
pub use pitch::{pitch_to_playback_rate, step_duration_secs};
pub use sample::{SampleBuffer, decode_sample};

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::Duration;

use rustfft::num_complex::Complex32;

use system_capture::SystemCapture;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::project::{Note, ProjectFile};

/// One Playlist placement's audio-thread-ready data: the step range it
/// occupies in the song timeline, plus that placement's pattern's channel
/// content (same shape `build_snapshot` produces for pattern mode).
pub struct SongPlacementVoiceData {
    pub start_step: u32,
    pub length_steps: u32,
    pub channels: Vec<ChannelVoiceData>,
}

pub struct SongSnapshot {
    pub placements: Vec<SongPlacementVoiceData>,
    pub song_length_steps: u32,
}

/// Builds the song-mode schedule from every Playlist placement, simplified
/// per `stages/06-playlist.md`: each placement's length is its pattern's
/// *nominal* `length_beats`, not the original's content-driven "longest
/// channel's reach" computation. Noted as a deliberate simplification in
/// `PROGRESS.md`, not a forgotten feature.
pub fn build_song_snapshot(project: &ProjectFile, samples: &HashMap<String, Arc<SampleBuffer>>) -> SongSnapshot {
    let any_soloed = project.channels.items.iter().any(|c| c.soloed);
    let steps_per_bar = project.beats_per_bar.max(1) * 4;

    let mut placements = Vec::new();
    let mut song_length_steps: u32 = 0;

    for placement in &project.placements.items {
        let Some(pattern) = project.patterns.items.iter().find(|p| p.id == placement.pattern_id) else { continue };
        let length_steps = (pattern.length_beats * 4.0).round().max(1.0) as u32;
        let start_step = (placement.start_beat * 4.0).round().max(0.0) as u32;

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
                    bus: ch.mixer_track,
                })
            })
            .collect();

        song_length_steps = song_length_steps.max(start_step + length_steps);
        placements.push(SongPlacementVoiceData { start_step, length_steps, channels });
    }

    // Round the song's length up to a whole bar; a totally empty
    // arrangement still reports one bar rather than zero.
    song_length_steps = ((song_length_steps.max(1) + steps_per_bar - 1) / steps_per_bar) * steps_per_bar;

    SongSnapshot { placements, song_length_steps }
}

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
    /// Which mixer bus this channel's voices render into — `0` means
    /// straight to Master (the default, per `stages/07-mixer.md`:
    /// "every channel routes to Master until explicitly assigned
    /// elsewhere"), otherwise a `MixerTrack` id.
    pub bus: u32,
}

pub struct PlaybackSnapshot {
    pub channels: Vec<ChannelVoiceData>,
    pub pattern_length_steps: u32,
    /// Project-wide, not pattern-scoped, but piggybacked onto this
    /// snapshot (rather than its own message) since `push_snapshot` already
    /// rebuilds it on every edit, including `set_beats_per_bar` — see
    /// `AudioEngine::set_metronome`'s doc comment for how this is used.
    pub steps_per_bar: u32,
}

/// One mixer-bus strip's audio-thread-ready data (Master itself is
/// handled separately — it has no mute/solo per `docs/mixer.md`'s
/// documented "Master is never mutable/soloable" rule, just the
/// `master_volume` carried alongside this list).
pub struct MixerBusVoiceData {
    pub id: u32,
    pub volume: f32,
    pub pan: f32,
    /// Already folded in: explicit mute, or (any bus soloed and this one
    /// isn't) — same "mute always wins" rule as channels, but *multi*-solo
    /// here (unlike the Channel Rack's single-solo), per the original's
    /// documented "standard multi-solo mixer semantics."
    pub muted: bool,
}

/// Builds the mixer-bus list + Master volume from the project — called
/// alongside `build_snapshot`/`build_song_snapshot` on every edit, since
/// routing/volume/pan/mute/solo on a bus affects both pattern and song
/// mode identically and isn't pattern-scoped content.
pub fn build_mixer_buses(project: &ProjectFile) -> (Vec<MixerBusVoiceData>, f32) {
    let any_soloed = project.mixer_tracks.items.iter().any(|t| t.soloed);
    let buses = project
        .mixer_tracks
        .items
        .iter()
        .map(|t| MixerBusVoiceData {
            id: t.id,
            volume: t.volume as f32,
            pan: t.pan as f32,
            muted: t.muted || (any_soloed && !t.soloed),
        })
        .collect();
    (buses, project.master_volume as f32)
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
                bus: ch.mixer_track,
            })
        })
        .collect();

    Ok(PlaybackSnapshot { channels, pattern_length_steps, steps_per_bar: project.beats_per_bar.max(1) * 4 })
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
    /// `0` = Master. Resolved to an accumulator slot each sample via
    /// `bus_index` rather than stored as an index, so a bus that's
    /// removed mid-note just falls back to Master instead of panicking
    /// on a stale index.
    bus: u32,
}

impl Voice {
    fn new(
        sample: Arc<SampleBuffer>,
        pitch_rate: f64,
        volume: f32,
        pan: f32,
        envelope: Envelope,
        out_sample_rate: u32,
        bus: u32,
    ) -> Self {
        let rate_correction = sample.sample_rate as f64 / out_sample_rate as f64;
        Voice { sample, pos: 0.0, playback_rate: pitch_rate * rate_correction, volume, pan, envelope, frames_played: 0, bus }
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

/// Shared by pattern mode and song mode: checks one channel's step/note
/// content against a single step index and schedules any voice it
/// triggers. `step` is pattern-relative in pattern mode, placement-local
/// in song mode — the caller picks which. No per-channel step-tiling here
/// (a channel's `steps` is a fixed 16-length pattern-mode row; stepping
/// past it in song mode for a longer placement just plays silence) —
/// that's the original's "per-channel independent looping", still
/// deferred per `02-feature-scope.md` in both modes, not a new gap this
/// stage introduces.
fn trigger_channel_step(ch: &ChannelVoiceData, step: i64, bpm: f64, out_sample_rate: u32, voices: &mut Vec<Voice>) {
    if ch.muted || step < 0 {
        return;
    }
    if ch.notes.is_empty() {
        if ch.steps.get(step as usize).copied().unwrap_or(false) {
            voices.push(Voice::new(
                ch.sample.clone(),
                1.0,
                ch.volume,
                ch.pan,
                Envelope::ring(out_sample_rate),
                out_sample_rate,
                ch.bus,
            ));
        }
    } else {
        for note in &ch.notes {
            if note.start.round() as i64 == step {
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
                    ch.bus,
                ));
            }
        }
    }
}

/// Synthesizes a short decaying sine-burst click for the metronome, rather
/// than shipping/downloading a pre-recorded sample — the only sound this
/// engine needs that isn't user-provided content. The decay is baked
/// directly into the sample data (down to silence well before the buffer
/// ends) so it can play through `Voice`/`Envelope::ring` exactly like any
/// other one-shot, with no separate envelope concept for metronome voices.
fn synth_click(sample_rate: u32, freq: f32) -> Arc<SampleBuffer> {
    const DURATION_SECS: f32 = 0.05;
    const DECAY_TAU_SECS: f32 = 0.012;
    let frame_count = (DURATION_SECS * sample_rate as f32) as usize;
    let data: Vec<f32> = (0..frame_count)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (2.0 * std::f32::consts::PI * freq * t).sin() * (-t / DECAY_TAU_SECS).exp()
        })
        .collect();
    Arc::new(SampleBuffer { channels: 1, sample_rate, data })
}

/// Resolves a channel/voice's `bus` id to a slot in the per-frame
/// accumulator: an unknown or `0` id falls back to `master_idx` (the
/// default "straight to Master" behavior, and a safety net if a bus was
/// removed while a voice targeting it was still ringing out).
fn bus_index(buses: &[MixerBusVoiceData], bus_id: u32, master_idx: usize) -> usize {
    if bus_id == 0 {
        return master_idx;
    }
    buses.iter().position(|b| b.id == bus_id).unwrap_or(master_idx)
}

struct PlaybackState {
    playing: AtomicBool,
    bpm_centibeats: AtomicU32,
    samples_played: AtomicU64,
    sample_rate: AtomicU32,
    pattern_length_steps: AtomicU32,
    /// Independent of `playing` — toggling Pattern/Song mode is a UI choice
    /// that can happen whether or not the transport is running; `play()`
    /// just starts the transport in whichever mode this is currently set
    /// to.
    song_mode: AtomicBool,
    song_length_steps: AtomicU32,
    /// Toolbar's metronome toggle — read once per new scheduled step inside
    /// the audio callback, same live-flag convention as `playing`/`song_
    /// mode`, rather than threaded through as a per-snapshot field (it has
    /// no content to rebuild, just an on/off read).
    metronome: AtomicBool,
}

/// Toolbar's audio monitor + peak meter (`src/toolbar.rs`'s `oscilloscope`
/// and `peak_meter`) — real master-bus metering, not the placeholder
/// "no hook yet" chrome this rewrite shipped with before. All three
/// fields are written from inside the audio callback every output frame,
/// so they're plain atomics (an `f32` stored as its bit pattern via
/// `to_bits`/`from_bits`, same trick `bpm_centibeats` already uses for
/// BPM) rather than anything that could block that thread.
struct MeterState {
    /// Windowed max `|sample|` since the last `take_frame_peak` call —
    /// see that method's doc comment.
    peak_l: AtomicU32,
    peak_r: AtomicU32,
    /// A non-decimated ring buffer of the mono mix, shared by both the
    /// oscilloscope and the spectrum analyzer — exactly how
    /// Allodium-tauri's own `BlackBox.svelte` reads both views off the
    /// same `AnalyserNode` (`getByteTimeDomainData`/`getByteFrequencyData`
    /// against one buffer). An earlier version here gave the oscilloscope
    /// its own *decimated* (16x) ring buffer sized for "tens of
    /// milliseconds," on the theory that the spectrum analyzer needed
    /// full resolution but the oscilloscope didn't. That was wrong: at
    /// 44.1kHz, 16x decimation drops the effective sample rate to
    /// ~2.7kHz, well under Nyquist for anything above ~1.4kHz — a user
    /// report ("a perfect sine wave shows up bit-crushed") confirmed it
    /// visibly aliased ordinary tones into a jagged trace. Reading the
    /// same full-rate buffer the spectrum analyzer already needed fixes
    /// this for free and removes a whole field's worth of bookkeeping.
    fft_buf: [AtomicU32; FFT_LEN],
    fft_write: AtomicUsize,
    /// The per-channel counterpart to `fft_buf`, same write cursor (so
    /// index `i` here and in `fft_buf` are always the same output
    /// frame) — the toolbar's XY (Lissajous) oscilloscope's source:
    /// plotting `(xy_l[i], xy_r[i])` as a point per frame is how an
    /// actual X-Y oscilloscope (and "oscilloscope music," tracks
    /// specifically authored to draw a shape when their two channels
    /// drive a scope's X/Y inputs instead of being played as stereo
    /// audio) reads a stereo signal. Not part of Allodium-tauri — a new
    /// addition, not a port — so there's no original `BlackBox.svelte`
    /// code to match here.
    xy_l: [AtomicU32; FFT_LEN],
    xy_r: [AtomicU32; FFT_LEN],
    /// The spectrum analyzer's already-binned (`bin_edge`-bucketed, but
    /// *not* autoscaled — `toolbar::ToolbarState::freq_gain` still owns
    /// that) magnitudes, written by a dedicated background thread
    /// (`spawn_spectrum_worker`) at its own fixed cadence, independent of
    /// render rate. Moving the FFT off the render path entirely — rather
    /// than throttling how often it ran there — is what actually fixed
    /// spectrum mode's lag; see that function's doc comment.
    spectrum_bins: [AtomicU32; SPECTRUM_BARS],
}

/// Public so `toolbar::spectrogram` can size its own `rustfft` plan to
/// match. A power of two (`rustfft` doesn't require it, but it's the
/// efficient case) spanning ~23ms at 44.1kHz — short enough to feel live,
/// long enough for the exponential-bucket bar mapping (`toolbar::
/// bin_edge`) to have real low-frequency resolution to work with.
pub(crate) const FFT_LEN: usize = 1024;

/// How many bars `toolbar::spectrogram` draws — also the width of
/// `MeterState::spectrum_bins`, since the background spectrum worker
/// (`spawn_spectrum_worker`) now does the exponential bucketing itself
/// (see `bin_edge`) rather than handing the render path raw FFT bins.
pub(crate) const SPECTRUM_BARS: usize = 44;

impl MeterState {
    fn new() -> Self {
        Self {
            peak_l: AtomicU32::new(0),
            peak_r: AtomicU32::new(0),
            fft_buf: std::array::from_fn(|_| AtomicU32::new(0)),
            fft_write: AtomicUsize::new(0),
            xy_l: std::array::from_fn(|_| AtomicU32::new(0)),
            xy_r: std::array::from_fn(|_| AtomicU32::new(0)),
            spectrum_bins: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }

    fn bump_max(slot: &AtomicU32, level: f32) {
        let mut current = slot.load(Ordering::Relaxed);
        loop {
            if level <= f32::from_bits(current) {
                return;
            }
            match slot.compare_exchange_weak(current, level.to_bits(), Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }

    /// Called once per output sample-frame from the audio callback with
    /// that frame's final (post-soft-clip) mix.
    fn push_frame(&self, mix_l: f32, mix_r: f32) {
        Self::bump_max(&self.peak_l, mix_l.abs());
        Self::bump_max(&self.peak_r, mix_r.abs());

        let mono = (mix_l + mix_r) * 0.5;

        let fft_idx = self.fft_write.fetch_add(1, Ordering::Relaxed) % FFT_LEN;
        self.fft_buf[fft_idx].store(mono.to_bits(), Ordering::Relaxed);
        self.xy_l[fft_idx].store(mix_l.to_bits(), Ordering::Relaxed);
        self.xy_r[fft_idx].store(mix_r.to_bits(), Ordering::Relaxed);
    }

    /// The max `|sample|` seen on each channel since the *previous* call
    /// to this method — a windowed instantaneous peak, consumed on read
    /// (reset to 0 as part of the same atomic swap), not a smoothed
    /// envelope. Matches Allodium-tauri's own split: its `peakOf` is this
    /// same per-poll raw max, and the attack/release envelope
    /// (`PeakMeter.svelte`'s `ampL = peakL > ampL ? peakL : ampL *
    /// RELEASE`) is applied by the *caller*, once per animation frame —
    /// not something that belongs at audio-sample rate, which has no
    /// well-defined "per frame" to release across. `toolbar::
    /// ToolbarState` is this app's equivalent caller.
    fn take_frame_peak(&self) -> (f32, f32) {
        let l = f32::from_bits(self.peak_l.swap(0.0f32.to_bits(), Ordering::Relaxed));
        let r = f32::from_bits(self.peak_r.swap(0.0f32.to_bits(), Ordering::Relaxed));
        (l, r)
    }

    /// The last `FFT_LEN` raw (non-decimated) samples, oldest first — the
    /// shared input window for both the oscilloscope and the spectrum
    /// analyzer.
    fn fft_snapshot(&self) -> Vec<f32> {
        let write = self.fft_write.load(Ordering::Relaxed);
        (0..FFT_LEN).map(|i| f32::from_bits(self.fft_buf[(write + i) % FFT_LEN].load(Ordering::Relaxed))).collect()
    }

    /// The last `FFT_LEN` `(l, r)` sample pairs, oldest first, index-
    /// aligned with `fft_snapshot` (same write cursor) — the XY
    /// oscilloscope's source.
    fn xy_snapshot(&self) -> (Vec<f32>, Vec<f32>) {
        let write = self.fft_write.load(Ordering::Relaxed);
        let l = (0..FFT_LEN).map(|i| f32::from_bits(self.xy_l[(write + i) % FFT_LEN].load(Ordering::Relaxed))).collect();
        let r = (0..FFT_LEN).map(|i| f32::from_bits(self.xy_r[(write + i) % FFT_LEN].load(Ordering::Relaxed))).collect();
        (l, r)
    }

    /// Called by `spawn_spectrum_worker`'s background thread only.
    fn write_spectrum_bins(&self, bins: &[f32]) {
        for (slot, v) in self.spectrum_bins.iter().zip(bins) {
            slot.store(v.to_bits(), Ordering::Relaxed);
        }
    }

    /// The spectrum analyzer's most recent already-binned magnitudes —
    /// cheap enough (`SPECTRUM_BARS` atomic loads) to call every render,
    /// unlike the FFT that produces them.
    fn spectrum_bins_snapshot(&self) -> Vec<f32> {
        self.spectrum_bins.iter().map(|b| f32::from_bits(b.load(Ordering::Relaxed))).collect()
    }
}

/// Real linear-amplitude magnitude per positive-frequency FFT bin (length
/// `samples.len() / 2`, DC..Nyquist) — a Hann-windowed (reduces spectral
/// leakage into neighboring bins) forward FFT via the cached plan in
/// `fft`, normalized by `len/2` so a full-scale sinusoid's own bin reads
/// back out at roughly its original time-domain amplitude (the same units
/// `toolbar`'s `freq_gain` floor is calibrated against).
pub(crate) fn spectrum_magnitude(samples: &[f32], fft: &dyn rustfft::Fft<f32>) -> Vec<f32> {
    let n = samples.len();
    let mut buf: Vec<Complex32> = samples
        .iter()
        .enumerate()
        .map(|(i, s)| {
            // Hann window.
            let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos();
            Complex32::new(s * w, 0.0)
        })
        .collect();
    fft.process(&mut buf);
    let norm = n as f32 / 2.0;
    buf[..n / 2].iter().map(|c| c.norm() / norm).collect()
}

/// Maps bar `i` of `bar_count` onto a bin index, exponentially rather than
/// linearly — Allodium-tauri's own `binEdge`, ported verbatim: "exponential
/// bucket edges so low frequencies (where most musical energy sits) get
/// more than the first pixel or two of a linear bin->pixel map."
pub(crate) fn bin_edge(bar: usize, bar_count: usize, bins: usize) -> usize {
    let frac = bar as f32 / bar_count as f32;
    (bins as f32).powf(frac).round().clamp(1.0, (bins - 1) as f32) as usize
}

/// Runs the spectrum analyzer's FFT on a dedicated background thread at a
/// fixed ~30Hz cadence, decoupled from render rate entirely — the original
/// Allodium-tauri's equivalent (`AnalyserNode`) is computed by the
/// browser's own audio pipeline, off the UI's main thread, which is why
/// its spectrum view never cost that app any frame time at all.
///
/// This rewrite's first spectrum implementation ran the FFT inline during
/// `toolbar::render_toolbar` and only throttled *how often* (every third
/// render) — see `PROGRESS.md`'s "click-to-switch spectrum analyzer"
/// follow-up. That still measured 9-16fps whenever the throttled frame
/// landed, under an unconfirmed "SIMD/AVX frequency-or-power-state
/// transition tax" theory: the FFT call's own wall-clock cost was
/// microseconds, yet the *frame that ran it* cost tens of milliseconds
/// more than frames that didn't, as if running vectorized code briefly
/// throttled the rest of that frame's GPU paint work. Whether or not
/// that's the exact mechanism, moving the FFT off the render path
/// entirely sidesteps it the same way the browser's `AnalyserNode` does:
/// the render path now only ever does `SPECTRUM_BARS` atomic loads plus
/// the existing autoscale (`toolbar::ToolbarState::freq_gain`), regardless
/// of how expensive any given FFT call happens to be on this machine.
fn spawn_spectrum_worker(meter: Arc<MeterState>) {
    thread::spawn(move || {
        let fft = rustfft::FftPlanner::new().plan_fft_forward(FFT_LEN);
        loop {
            thread::sleep(Duration::from_millis(33));
            let raw = meter.fft_snapshot();
            let magnitudes = spectrum_magnitude(&raw, fft.as_ref());
            let n = magnitudes.len();
            let bins: Vec<f32> = (0..SPECTRUM_BARS)
                .map(|i| {
                    let lo = bin_edge(i, SPECTRUM_BARS, n);
                    let hi = bin_edge(i + 1, SPECTRUM_BARS, n).max(lo + 1).min(n);
                    magnitudes.get(lo..hi).map(|s| s.iter().cloned().fold(0.0f32, f32::max)).unwrap_or(0.0)
                })
                .collect();
            meter.write_spectrum_bins(&bins);
        }
    });
}

/// Sent from the UI thread into the running audio callback. Applied at
/// the top of each buffer (not per-frame — up to one buffer's worth of
/// latency, a few milliseconds, which is imperceptible for a UI edit).
enum EngineMessage {
    /// Replaces the content the scheduler plays from — pushed on every
    /// step/volume/pan/mute/solo/channel-list edit so changes take effect
    /// immediately without interrupting playback position.
    UpdateContent { channels: Vec<ChannelVoiceData>, pattern_length_steps: u32, steps_per_bar: u32 },
    /// A one-shot outside the scheduler, for click-to-audition. Plays
    /// regardless of transport state.
    Preview { sample: Arc<SampleBuffer>, volume: f32, pan: f32 },
    /// Replaces the Playlist's song-mode schedule. Pushed on every project
    /// edit alongside `UpdateContent` (patterns, placements, channel
    /// content can all affect it), same reasoning as that variant.
    UpdateSongContent { placements: Vec<SongPlacementVoiceData>, song_length_steps: u32 },
    /// Replaces the mixer-bus list + Master volume. Applies to both
    /// pattern and song mode identically, so it's pushed independently of
    /// either rather than duplicated into both snapshot kinds.
    UpdateMixer { buses: Vec<MixerBusVoiceData>, master_volume: f32 },
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
    meter: Arc<MeterState>,
    /// "System audio" monitor source — see `system_capture.rs`. A
    /// separate `MeterState` from `meter` (which only ever reflects this
    /// engine's own mix), started/stopped by `{start,stop}_
    /// system_capture` in response to the toolbar's monitor-source
    /// toggle, not running by default.
    system_meter: Arc<MeterState>,
    system_capture: Mutex<Option<SystemCapture>>,
    system_error: Mutex<Option<String>>,
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
            song_mode: AtomicBool::new(false),
            song_length_steps: AtomicU32::new(16),
            metronome: AtomicBool::new(false),
        });

        let meter = Arc::new(MeterState::new());

        // One quiet regular click and one louder, higher-pitched accent —
        // distinguished by pitch/volume rather than a second recorded
        // sample, since both are synthesized (see `synth_click`).
        let click_sample = synth_click(out_sample_rate, 1500.0);
        let accent_sample = synth_click(out_sample_rate, 2200.0);

        let (sender, receiver) = mpsc::channel::<EngineMessage>();
        let callback_state = state.clone();
        let callback_meter = meter.clone();
        let mut channels: Vec<ChannelVoiceData> = Vec::new();
        let mut pattern_length_steps: i64 = 16;
        let mut steps_per_bar: i64 = 16;
        let mut song_placements: Vec<SongPlacementVoiceData> = Vec::new();
        let mut song_length_steps: i64 = 16;
        let mut voices: Vec<Voice> = Vec::new();
        let mut last_triggered_step: i64 = -1;
        let mut buses: Vec<MixerBusVoiceData> = Vec::new();
        let mut master_volume: f32 = 1.0;
        // Per-bus accumulators for one output sample-frame; index
        // `buses.len()` is reserved for Master. Resized only when `buses`
        // changes (on an `UpdateMixer` message), zeroed every frame —
        // no per-sample allocation.
        let mut bus_l: Vec<f32> = vec![0.0];
        let mut bus_r: Vec<f32> = vec![0.0];

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    for msg in receiver.try_iter() {
                        match msg {
                            EngineMessage::UpdateContent {
                                channels: new_channels,
                                pattern_length_steps: new_len,
                                steps_per_bar: new_bar,
                            } => {
                                channels = new_channels;
                                pattern_length_steps = new_len.max(1) as i64;
                                steps_per_bar = new_bar.max(1) as i64;
                            }
                            EngineMessage::Preview { sample, volume, pan } => {
                                voices.push(Voice::new(sample, 1.0, volume, pan, Envelope::ring(out_sample_rate), out_sample_rate, 0));
                            }
                            EngineMessage::UpdateSongContent { placements, song_length_steps: new_len } => {
                                song_placements = placements;
                                song_length_steps = new_len.max(1) as i64;
                            }
                            EngineMessage::UpdateMixer { buses: new_buses, master_volume: new_master } => {
                                buses = new_buses;
                                master_volume = new_master;
                                bus_l = vec![0.0; buses.len() + 1];
                                bus_r = vec![0.0; buses.len() + 1];
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
                            let is_new_step = whole_step != last_triggered_step;

                            // One click per quarter note (every 4 sixteenth
                            // steps, the fixed step convention everywhere
                            // else in this engine), accented on beat 1 of
                            // every bar — ticks on the absolute step clock
                            // so it stays steady across a pattern loop or a
                            // song-mode arrangement alike, independent of
                            // the mode-specific triggering below. Read
                            // before either branch below updates
                            // `last_triggered_step`, so it never
                            // double-fires on the same step.
                            if is_new_step && callback_state.metronome.load(Ordering::Relaxed) && whole_step >= 0 {
                                if whole_step % steps_per_bar == 0 {
                                    voices.push(Voice::new(
                                        accent_sample.clone(),
                                        1.0,
                                        0.9,
                                        0.5,
                                        Envelope::ring(out_sample_rate),
                                        out_sample_rate,
                                        0,
                                    ));
                                } else if whole_step % 4 == 0 {
                                    voices.push(Voice::new(
                                        click_sample.clone(),
                                        1.0,
                                        0.7,
                                        0.5,
                                        Envelope::ring(out_sample_rate),
                                        out_sample_rate,
                                        0,
                                    ));
                                }
                            }

                            if callback_state.song_mode.load(Ordering::Relaxed) {
                                // Song mode: play the arrangement once,
                                // stopping cleanly at the end rather than
                                // looping (matches the original's
                                // documented "stop-at-end" behavior).
                                if whole_step >= song_length_steps {
                                    callback_state.playing.store(false, Ordering::Relaxed);
                                } else if whole_step != last_triggered_step {
                                    last_triggered_step = whole_step;
                                    for placement in &song_placements {
                                        let start = placement.start_step as i64;
                                        let end = start + placement.length_steps as i64;
                                        if whole_step < start || whole_step >= end {
                                            continue;
                                        }
                                        let local_step = whole_step - start;
                                        for ch in &placement.channels {
                                            trigger_channel_step(ch, local_step, bpm, out_sample_rate, &mut voices);
                                        }
                                    }
                                }
                            } else if whole_step != last_triggered_step {
                                last_triggered_step = whole_step;
                                let pattern_step = whole_step.rem_euclid(pattern_length_steps);
                                for ch in &channels {
                                    trigger_channel_step(ch, pattern_step, bpm, out_sample_rate, &mut voices);
                                }
                            }
                        }

                        // Voices (scheduled or previewed) keep rendering
                        // regardless of transport state, so a preview
                        // click plays out even while stopped.
                        let master_idx = buses.len();
                        for x in bus_l.iter_mut() {
                            *x = 0.0;
                        }
                        for x in bus_r.iter_mut() {
                            *x = 0.0;
                        }
                        voices.retain_mut(|v| {
                            let mut l = 0.0f32;
                            let mut r = 0.0f32;
                            let alive = v.render(&mut l, &mut r);
                            let idx = bus_index(&buses, v.bus, master_idx);
                            bus_l[idx] += l;
                            bus_r[idx] += r;
                            alive
                        });

                        // Mix every non-Master bus's own volume/pan/mute
                        // down into Master — a channel routed straight to
                        // Master (`bus == 0`, the default) already landed
                        // in `master_idx` above and skips this entirely,
                        // so only an explicitly-routed channel's sound is
                        // affected by its bus's controls, independent of
                        // the channel's own volume/pan/mute.
                        for (i, bus) in buses.iter().enumerate() {
                            if bus.muted {
                                continue;
                            }
                            let bl = bus_l[i] * bus.volume;
                            let br = bus_r[i] * bus.volume;
                            // A balance control, not the channel-level
                            // equal-power pan above — the source here is
                            // already a stereo sub-mix, not a mono
                            // sample being positioned in the field.
                            let pan = bus.pan.clamp(0.0, 1.0);
                            let left_gain = 1.0 - (pan - 0.5).max(0.0) * 2.0;
                            let right_gain = 1.0 - (0.5 - pan).max(0.0) * 2.0;
                            bus_l[master_idx] += bl * left_gain;
                            bus_r[master_idx] += br * right_gain;
                        }

                        // Soft clip rather than a hard clamp, so a loud
                        // mix compresses gracefully instead of crackling.
                        let mix_l = (bus_l[master_idx] * master_volume).tanh();
                        let mix_r = (bus_r[master_idx] * master_volume).tanh();
                        callback_meter.push_frame(mix_l, mix_r);

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

        let system_meter = Arc::new(MeterState::new());
        spawn_spectrum_worker(meter.clone());
        spawn_spectrum_worker(system_meter.clone());

        Ok(Self {
            _stream: stream,
            sender,
            state,
            meter,
            system_meter,
            system_capture: Mutex::new(None),
            system_error: Mutex::new(None),
        })
    }

    pub fn is_playing(&self) -> bool {
        self.state.playing.load(Ordering::Relaxed)
    }

    pub fn set_bpm(&self, bpm: f64) {
        self.state.bpm_centibeats.store((bpm * 100.0).round() as u32, Ordering::Relaxed);
    }

    /// (L, R) max `|sample|` since the last call — see
    /// `MeterState::take_frame_peak`. Call once per UI frame; the caller
    /// (`toolbar::ToolbarState`) applies its own attack/release envelope
    /// on top, at render cadence.
    pub fn peak_levels(&self) -> (f32, f32) {
        self.meter.take_frame_peak()
    }

    /// The toolbar oscilloscope's most recent window of the master mix,
    /// oldest sample first — the same raw, non-decimated buffer the
    /// spectrum analyzer reads, so the oscilloscope renders at full
    /// sample-rate resolution.
    pub fn scope_snapshot(&self) -> Vec<f32> {
        self.meter.fft_snapshot()
    }

    /// The toolbar XY oscilloscope's most recent `(l, r)` sample pairs,
    /// oldest first, index-aligned with `scope_snapshot`.
    pub fn xy_snapshot(&self) -> (Vec<f32>, Vec<f32>) {
        self.meter.xy_snapshot()
    }

    /// The toolbar spectrum analyzer's most recent already-binned
    /// magnitudes (`SPECTRUM_BARS` long), produced by a background thread
    /// — see `spawn_spectrum_worker`. Cheap to call every render.
    pub fn spectrum_bins(&self) -> Vec<f32> {
        self.meter.spectrum_bins_snapshot()
    }

    /// Starts "System audio" capture (see `system_capture.rs`) if it
    /// isn't already running. Replaces any previous failure recorded in
    /// `system_capture_error` with the result of this attempt.
    pub fn start_system_capture(&self) {
        let mut capture = self.system_capture.lock().unwrap();
        if capture.is_some() {
            return;
        }
        match SystemCapture::start(self.system_meter.clone()) {
            Ok(c) => {
                *capture = Some(c);
                *self.system_error.lock().unwrap() = None;
            }
            Err(e) => *self.system_error.lock().unwrap() = Some(e),
        }
    }

    /// Stops "System audio" capture (dropping it kills the `pw-record`
    /// child process) and clears any recorded error.
    pub fn stop_system_capture(&self) {
        *self.system_capture.lock().unwrap() = None;
        *self.system_error.lock().unwrap() = None;
    }

    pub fn system_peak_levels(&self) -> (f32, f32) {
        self.system_meter.take_frame_peak()
    }

    pub fn system_scope_snapshot(&self) -> Vec<f32> {
        self.system_meter.fft_snapshot()
    }

    pub fn system_xy_snapshot(&self) -> (Vec<f32>, Vec<f32>) {
        self.system_meter.xy_snapshot()
    }

    pub fn system_spectrum_bins(&self) -> Vec<f32> {
        self.system_meter.spectrum_bins_snapshot()
    }

    /// Why `start_system_capture` last failed, if it did — e.g. PipeWire's
    /// CLI tools aren't on `PATH`. `None` once capture is running (or
    /// before it's ever been attempted).
    pub fn system_capture_error(&self) -> Option<String> {
        self.system_error.lock().unwrap().clone()
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
            steps_per_bar: snapshot.steps_per_bar,
        });
    }

    /// Toolbar's metronome toggle. Takes effect on the very next scheduled
    /// step, no restart needed — same live-flag convention as `set_bpm`.
    pub fn set_metronome(&self, enabled: bool) {
        self.state.metronome.store(enabled, Ordering::Relaxed);
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

    /// Pushes a fresh song-mode schedule — call on every edit that could
    /// affect it (placements, patterns, or any channel content), same
    /// "always push, let the callback hold the latest" convention as
    /// `set_content`.
    pub fn set_song_content(&self, snapshot: SongSnapshot) {
        self.state.song_length_steps.store(snapshot.song_length_steps.max(1), Ordering::Relaxed);
        let _ = self.sender.send(EngineMessage::UpdateSongContent {
            placements: snapshot.placements,
            song_length_steps: snapshot.song_length_steps,
        });
    }

    /// Pushes a fresh mixer-bus list + Master volume — call on every edit
    /// that could affect it (a bus's own volume/pan/mute/solo, adding/
    /// removing a bus, or a channel's routing).
    pub fn set_mixer(&self, buses: Vec<MixerBusVoiceData>, master_volume: f32) {
        let _ = self.sender.send(EngineMessage::UpdateMixer { buses, master_volume });
    }

    /// Selects which mode `play()` starts in. Independent of `playing` —
    /// can be toggled whether the transport is running or stopped; if it's
    /// running, the caller is responsible for restarting it (stop + play)
    /// so the switch takes effect immediately rather than on the next
    /// manual play, mirroring the original's "flipping the toggle while
    /// playing restarts in the new mode" behavior.
    pub fn set_song_mode(&self, enabled: bool) {
        self.state.song_mode.store(enabled, Ordering::Relaxed);
    }

    pub fn is_song_mode(&self) -> bool {
        self.state.song_mode.load(Ordering::Relaxed)
    }

    /// Continuous, unwrapped step position since the transport started —
    /// the Song-mode equivalent of `current_step()`'s wrapped pattern
    /// position, for the Playlist's playhead.
    pub fn song_step(&self) -> f64 {
        let sr = self.state.sample_rate.load(Ordering::Relaxed).max(1) as f64;
        let bpm = self.state.bpm_centibeats.load(Ordering::Relaxed) as f64 / 100.0;
        let samples = self.state.samples_played.load(Ordering::Relaxed) as f64;
        let steps_per_sec = bpm / 60.0 * 4.0;
        samples / sr * steps_per_sec
    }

    /// Jumps the transport clock to `step` — the Playlist ruler's
    /// click-to-seek. Works whether playing or stopped (the scheduler
    /// picks up the new position on its very next frame either way, since
    /// it only ever reads `samples_played` fresh); has no lasting effect
    /// if stopped, though, since `play()` always resets to step 0.
    pub fn seek_to_step(&self, step: f64) {
        let sr = self.state.sample_rate.load(Ordering::Relaxed).max(1) as f64;
        let bpm = self.state.bpm_centibeats.load(Ordering::Relaxed) as f64 / 100.0;
        let steps_per_sec = bpm / 60.0 * 4.0;
        let samples = (step.max(0.0) / steps_per_sec * sr).round() as u64;
        self.state.samples_played.store(samples, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod song_snapshot_tests {
    use super::*;
    use crate::project::{Pattern, Placement, ProjectFile};

    #[test]
    fn an_empty_arrangement_is_one_bar_long() {
        let project = ProjectFile::default();
        let snapshot = build_song_snapshot(&project, &HashMap::new());
        assert_eq!(snapshot.song_length_steps, 16);
        assert!(snapshot.placements.is_empty());
    }

    #[test]
    fn placement_steps_come_from_the_pattern_not_a_manual_length() {
        let mut project = ProjectFile::default();
        project.patterns.items.push(Pattern { id: 2, name: "B".to_string(), length_beats: 4.0 });
        project.placements.items.push(Placement { id: 1, pattern_id: 2, track_id: 0, start_beat: 4.0 });

        let snapshot = build_song_snapshot(&project, &HashMap::new());
        assert_eq!(snapshot.placements.len(), 1);
        assert_eq!(snapshot.placements[0].start_step, 16);
        assert_eq!(snapshot.placements[0].length_steps, 16);
        // End step 32 is already a whole-bar boundary at 16 steps/bar.
        assert_eq!(snapshot.song_length_steps, 32);
    }

    #[test]
    fn song_length_rounds_up_to_a_whole_bar() {
        let mut project = ProjectFile::default();
        // Default pattern 1 is 4 beats (16 steps); starting at beat 1
        // makes the arrangement end mid-bar (step 20), which must round
        // up to the next whole bar (32), not truncate to 16.
        project.placements.items.push(Placement { id: 1, pattern_id: 1, track_id: 0, start_beat: 1.0 });

        let snapshot = build_song_snapshot(&project, &HashMap::new());
        assert_eq!(snapshot.song_length_steps, 32);
    }
}

#[cfg(test)]
mod spectrum_tests {
    use super::*;

    /// A synthetic signal, not a live capture — unlike `pw-record`'s own
    /// round trip (checked interactively while building this, not as a
    /// committed test: a live loopback capture on this machine showed
    /// real low-frequency content *alongside* the test tone, which is a
    /// property of this machine's audio chain, not something either this
    /// function or that test could fix), this isolates `spectrum_
    /// magnitude`'s own math from anything the audio chain might do.
    #[test]
    fn spectrum_magnitude_peaks_near_the_right_bin() {
        let rate = 44_100.0f32;
        let freq = 440.0f32;
        let samples: Vec<f32> = (0..FFT_LEN).map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / rate).sin()).collect();
        let mut planner = rustfft::FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_LEN);
        let mags = spectrum_magnitude(&samples, fft.as_ref());

        let (peak_bin, _) = mags.iter().enumerate().fold((0, 0.0f32), |acc, (i, &m)| if m > acc.1 { (i, m) } else { acc });
        let bin_hz = rate / FFT_LEN as f32;
        let peak_hz = peak_bin as f32 * bin_hz;
        assert!((400.0..480.0).contains(&peak_hz), "expected the peak near 440Hz, got {peak_hz:.0}Hz");
    }

    #[test]
    fn bin_edge_is_monotonic_and_spans_the_full_bin_range() {
        let bins = 512;
        let mut prev = 0;
        for bar in 0..SPECTRUM_BARS {
            let edge = bin_edge(bar, SPECTRUM_BARS, bins);
            assert!(edge >= prev, "bin_edge should never go backwards (bar {bar}: {edge} < {prev})");
            prev = edge;
        }
        assert_eq!(bin_edge(SPECTRUM_BARS, SPECTRUM_BARS, bins), bins - 1, "the last edge should reach the top of the bin range");
    }
}

#[cfg(test)]
mod mixer_bus_tests {
    use super::*;
    use crate::project::{MixerTrack, ProjectFile};

    fn track(id: u32, muted: bool, soloed: bool) -> MixerTrack {
        MixerTrack { id, volume: 0.8, pan: 0.5, muted, soloed, routes_to: 0 }
    }

    #[test]
    fn an_explicit_mute_wins_even_if_nothing_else_is_soloed() {
        let mut project = ProjectFile::default();
        project.mixer_tracks.items.push(track(1, true, false));
        let (buses, _) = build_mixer_buses(&project);
        assert!(buses[0].muted);
    }

    #[test]
    fn soloing_one_track_silences_the_others_even_if_unmuted() {
        let mut project = ProjectFile::default();
        project.mixer_tracks.items.push(track(1, false, false));
        project.mixer_tracks.items.push(track(2, false, true));
        let (buses, _) = build_mixer_buses(&project);
        assert!(buses[0].muted, "track 1 isn't soloed and another track is, so it should be silenced");
        assert!(!buses[1].muted, "the soloed track itself must stay audible");
    }

    #[test]
    fn an_unknown_or_zero_bus_id_falls_back_to_master() {
        let mut project = ProjectFile::default();
        project.mixer_tracks.items.push(track(1, false, false));
        let (buses, _) = build_mixer_buses(&project);
        let master_idx = buses.len();
        assert_eq!(bus_index(&buses, 0, master_idx), master_idx);
        assert_eq!(bus_index(&buses, 99, master_idx), master_idx, "a removed/unknown bus id must not panic or alias another bus");
        assert_eq!(bus_index(&buses, 1, master_idx), 0);
    }
}
