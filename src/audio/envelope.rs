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
}

fn declick_frames(sample_rate: u32) -> u64 {
    (DECLICK_SECS * sample_rate as f32) as u64
}

impl Envelope {
    pub fn ring(sample_rate: u32) -> Self {
        Envelope::Ring { declick_frames: declick_frames(sample_rate) }
    }

    /// Decay/release stage lengths are clamped to fit inside very short
    /// notes so a 1-step note doesn't produce a negative sustain.
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

    /// `None` once the voice should be cut (only `Shaped` ever returns it).
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
}
