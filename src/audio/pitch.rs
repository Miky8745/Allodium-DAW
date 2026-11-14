//! Timing/pitch math, ported exactly from Allodium-tauri's conventions
//! (`docs/audio-playback.md`) so behavior matches rather than merely
//! resembling the original.

/// MIDI-style semitone pitch, `60` = unshifted (rate `1.0`), using the
/// `60 = C5` octave-numbering convention. Internal-only — there's no MIDI
/// import/export, so only self-consistency matters.
pub fn pitch_to_playback_rate(pitch: i32) -> f64 {
    2f64.powf((pitch - 60) as f64 / 12.0)
}

/// Seconds per sixteenth-note step at a given BPM (16 steps = 1 bar of 4/4).
pub fn step_duration_secs(bpm: f64) -> f64 {
    60.0 / bpm / 4.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unshifted_pitch_is_unity_rate() {
        assert_eq!(pitch_to_playback_rate(60), 1.0);
    }

    #[test]
    fn an_octave_up_doubles_the_rate() {
        assert!((pitch_to_playback_rate(72) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn an_octave_down_halves_the_rate() {
        assert!((pitch_to_playback_rate(48) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn step_duration_at_120_bpm_is_an_eighth_of_a_second() {
        // 120 BPM -> 2 beats/sec -> 8 sixteenth-steps/sec -> 0.125s/step.
        assert!((step_duration_secs(120.0) - 0.125).abs() < 1e-9);
    }
}
