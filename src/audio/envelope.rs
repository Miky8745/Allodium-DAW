//! ADSR-style envelope shaping, ported from Allodium-tauri's fixed
//! constants (`docs/audio-playback.md`'s "ADSR envelope on piano-roll
//! notes" — attack is `0` since samples already carry their own natural
//! attack; this only shapes decay/sustain/release). Per-channel/UI-exposed
//! envelope controls are stretch scope.

const DECAY_SECS: f32 = 0.05;
const SUSTAIN_LEVEL: f32 = 0.7;
const RELEASE_SECS: f32 = 0.05;
/// Not from the original — a short linear fade-in applied to every voice
/// (step one-shots included) so starting mid-waveform never clicks.
const DECLICK_SECS: f32 = 0.003;

pub enum Envelope {
    /// Step one-shot: ring out to the sample's own natural end, with just
    /// the de-click fade-in — a "length" concept doesn't apply, and
    /// cutting it short would regress drum-style hits.
    Ring { declick_frames: u64 },
    /// Piano-roll note: decay/sustain/release shaped to fit the note's
    /// own length, with an explicit cutoff — this is what makes a note
    /// actually stop when its length says to, instead of ringing to the
    /// sample's full recorded duration.
    Shaped { decay_frames: u64, sustain_frames: u64, release_frames: u64, declick_frames: u64 },
    /// A live-held note (QWERTY "typing keyboard to piano" / future MIDI
    /// input): decays to sustain and then holds there indefinitely —
    /// `release_at` starts `None` and stays that way until `release` is
    /// called (key-up), at which point it ramps to silence over
    /// `RELEASE_SECS` from whatever level it was at.
    Sustained { declick_frames: u64, decay_frames: u64, release_frames: u64, release_at: Option<u64> },
}

fn declick_frames(sample_rate: u32) -> u64 {
    (DECLICK_SECS * sample_rate as f32) as u64
}

/// `Note::release` (`0..1`) to an actual release-stage duration in
/// seconds: `0.5` is `RELEASE_SECS` (the old fixed default), linear down
/// to `0` at `release_param: 0.0` and up to 2 seconds at `release_param:
/// 1.0` — Allodium-tauri's own `releaseSeconds()`.
fn release_seconds(release_param: f64) -> f32 {
    const MAX_RELEASE_SECS: f32 = 2.0;
    let r = release_param.clamp(0.0, 1.0) as f32;
    if r <= 0.5 { (r / 0.5) * RELEASE_SECS } else { RELEASE_SECS + ((r - 0.5) / 0.5) * (MAX_RELEASE_SECS - RELEASE_SECS) }
}

impl Envelope {
    pub fn ring(sample_rate: u32) -> Self {
        Envelope::Ring { declick_frames: declick_frames(sample_rate) }
    }

    /// Decay/release stage lengths are clamped to fit inside very short
    /// notes so a 1-step note doesn't produce a negative sustain. Release
    /// always fits *inside* `duration_frames` here — this is the fixed
    /// `RELEASE_SECS` behavior from before per-note release existed;
    /// `shaped_with_release`'s own doc comment covers how a real
    /// `Note::release` value changes that.
    pub fn shaped(duration_frames: u64, sample_rate: u32) -> Self {
        let mut decay = (DECAY_SECS * sample_rate as f32) as u64;
        let mut release = (RELEASE_SECS * sample_rate as f32) as u64;
        if decay + release > duration_frames {
            let scale = duration_frames as f64 / (decay + release).max(1) as f64;
            decay = (decay as f64 * scale) as u64;
            release = (release as f64 * scale) as u64;
        }
        let sustain = duration_frames.saturating_sub(decay).saturating_sub(release);
        Envelope::Shaped {
            decay_frames: decay,
            sustain_frames: sustain,
            release_frames: release,
            declick_frames: declick_frames(sample_rate),
        }
    }

    /// Like `shaped`, but `release_param` (`Note::release`, `0..1`) picks
    /// how long the release tail actually is — and, unlike `shaped`'s own
    /// fixed `RELEASE_SECS`, that tail is appended *after*
    /// `duration_frames` rather than fit inside it, so a longer release
    /// really does ring on past the note's own scheduled end (up to ~2s
    /// at `release_param: 1.0`), matching Allodium-tauri's own per-note
    /// `release`. At exactly `0.5` (the default/sentinel — "the engine's
    /// old fixed ~50ms release") this delegates straight to `shaped`,
    /// preserving that exact pre-existing behavior rather than
    /// reinterpreting the same nominal value two different ways.
    pub fn shaped_with_release(duration_frames: u64, release_param: f64, sample_rate: u32) -> Self {
        if (release_param - 0.5).abs() < 1e-9 {
            return Self::shaped(duration_frames, sample_rate);
        }
        let decay = ((DECAY_SECS * sample_rate as f32) as u64).min(duration_frames.max(1));
        let sustain = duration_frames.saturating_sub(decay);
        let release = ((release_seconds(release_param) * sample_rate as f32) as u64).max(1);
        Envelope::Shaped { decay_frames: decay, sustain_frames: sustain, release_frames: release, declick_frames: declick_frames(sample_rate) }
    }

    pub fn sustained(sample_rate: u32) -> Self {
        Envelope::Sustained {
            declick_frames: declick_frames(sample_rate),
            decay_frames: (DECAY_SECS * sample_rate as f32) as u64,
            release_frames: (RELEASE_SECS * sample_rate as f32) as u64,
            release_at: None,
        }
    }

    /// Starts the release stage of a `Sustained` envelope at `frame`
    /// (idempotent — a second call once already releasing is a no-op, so
    /// a stray duplicate key-up can't restart the fade). No-op on any
    /// other variant.
    pub fn release(&mut self, frame: u64) {
        if let Envelope::Sustained { release_at, .. } = self {
            if release_at.is_none() {
                *release_at = Some(frame);
            }
        }
    }

    fn decay_shape(frame: u64, decay_frames: u64) -> f32 {
        if frame < decay_frames {
            let t = frame as f32 / decay_frames.max(1) as f32;
            1.0 - t * (1.0 - SUSTAIN_LEVEL)
        } else {
            SUSTAIN_LEVEL
        }
    }

    /// `None` once the voice should be cut (`Shaped` once its release
    /// ends; `Sustained` once *its* release — started by `release` —
    /// ends; `Ring` never does).
    pub fn gain_at(&self, frame: u64) -> Option<f32> {
        match *self {
            Envelope::Ring { declick_frames } => Some(declick(frame, declick_frames)),
            Envelope::Shaped { decay_frames, sustain_frames, release_frames, declick_frames } => {
                let shape = if frame < decay_frames {
                    let t = frame as f32 / decay_frames.max(1) as f32;
                    1.0 - t * (1.0 - SUSTAIN_LEVEL)
                } else if frame < decay_frames + sustain_frames {
                    SUSTAIN_LEVEL
                } else if frame < decay_frames + sustain_frames + release_frames {
                    let into_release = frame - decay_frames - sustain_frames;
                    let t = into_release as f32 / release_frames.max(1) as f32;
                    SUSTAIN_LEVEL * (1.0 - t)
                } else {
                    return None;
                };
                Some(declick(frame, declick_frames) * shape)
            }
            Envelope::Sustained { decay_frames, release_frames, declick_frames, release_at } => {
                let shape = match release_at {
                    None => Self::decay_shape(frame, decay_frames),
                    Some(r) if frame < r => Self::decay_shape(frame, decay_frames),
                    Some(r) => {
                        let level_at_release = Self::decay_shape(r, decay_frames);
                        let into_release = frame - r;
                        if into_release >= release_frames {
                            return None;
                        }
                        let t = into_release as f32 / release_frames.max(1) as f32;
                        level_at_release * (1.0 - t)
                    }
                };
                Some(declick(frame, declick_frames) * shape)
            }
        }
    }
}

fn declick(frame: u64, declick_frames: u64) -> f32 {
    if frame < declick_frames { frame as f32 / declick_frames.max(1) as f32 } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_envelope_never_cuts_off() {
        let env = Envelope::ring(48000);
        assert!(env.gain_at(0).is_some());
        assert!(env.gain_at(1_000_000).is_some());
    }

    #[test]
    fn shaped_envelope_cuts_off_at_its_duration() {
        let env = Envelope::shaped(4800, 48000); // 0.1s
        assert!(env.gain_at(4799).is_some());
        assert!(env.gain_at(4800).is_none());
    }

    #[test]
    fn very_short_note_still_has_a_valid_envelope() {
        // Shorter than decay+release (0.1s) combined.
        let env = Envelope::shaped(10, 48000);
        assert!(env.gain_at(0).is_some());
        assert!(env.gain_at(9).is_some());
        assert!(env.gain_at(10).is_none());
    }

    #[test]
    fn sustained_envelope_holds_until_released() {
        let env = Envelope::sustained(48000);
        assert!(env.gain_at(0).is_some());
        assert!(env.gain_at(1_000_000).is_some()); // still held, way past decay
    }

    #[test]
    fn sustained_envelope_cuts_off_after_release() {
        let mut env = Envelope::sustained(48000);
        env.release(1000);
        assert!(env.gain_at(999).is_some()); // not released yet at this frame
        assert!(env.gain_at(1000).is_some()); // release just started
        assert!(env.gain_at(1000 + (RELEASE_SECS * 48000.0) as u64).is_none());
    }

    #[test]
    fn a_second_release_call_does_not_restart_the_fade() {
        let mut env = Envelope::sustained(48000);
        env.release(1000);
        env.release(5000); // should be ignored — already releasing from 1000
        let cutoff = 1000 + (RELEASE_SECS * 48000.0) as u64;
        assert!(env.gain_at(cutoff).is_none());
    }
}
