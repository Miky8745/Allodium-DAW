//! Thin proxies to `AudioEngine` — playback state, transport, and the
//! toolbar's metering reads. Split out of `channel_rack.rs` because it's
//! the single largest group of near-identical one-line delegations
//! there; none of these touch `self.project` beyond reading `tempo` to
//! hand to the engine.

use gpui::Context;

use super::ChannelRack;

impl ChannelRack {
    pub fn is_playing(&self) -> bool {
        self.engine.is_playing()
    }

    pub fn set_metronome(&self, enabled: bool) {
        self.engine.set_metronome(enabled);
    }

    pub fn tempo(&self) -> f64 {
        self.project.tempo
    }

    /// Current (L, R) master-bus peak level, for the toolbar's peak meter.
    pub fn peak_levels(&self) -> (f32, f32) {
        self.engine.peak_levels()
    }

    /// The toolbar oscilloscope's most recent window of the master mix.
    pub fn scope_snapshot(&self) -> Vec<f32> {
        self.engine.scope_snapshot()
    }

    /// The toolbar XY oscilloscope's most recent `(l, r)` sample pairs.
    pub fn xy_snapshot(&self) -> (Vec<f32>, Vec<f32>) {
        self.engine.xy_snapshot()
    }

    /// The toolbar spectrum analyzer's most recent already-binned
    /// magnitudes of the master mix.
    pub fn spectrum_bins(&self) -> Vec<f32> {
        self.engine.spectrum_bins()
    }

    pub fn start_system_capture(&self) {
        self.engine.start_system_capture();
    }

    pub fn stop_system_capture(&self) {
        self.engine.stop_system_capture();
    }

    pub fn system_peak_levels(&self) -> (f32, f32) {
        self.engine.system_peak_levels()
    }

    pub fn system_scope_snapshot(&self) -> Vec<f32> {
        self.engine.system_scope_snapshot()
    }

    pub fn system_xy_snapshot(&self) -> (Vec<f32>, Vec<f32>) {
        self.engine.system_xy_snapshot()
    }

    pub fn system_spectrum_bins(&self) -> Vec<f32> {
        self.engine.system_spectrum_bins()
    }

    pub fn system_capture_error(&self) -> Option<String> {
        self.engine.system_capture_error()
    }

    /// Live-updates the engine's BPM immediately if playing (`set_bpm`),
    /// matching the original's scroll-to-adjust toolbar tempo display.
    pub fn set_tempo(&mut self, tempo: f64, cx: &mut Context<Self>) {
        self.project.tempo = tempo.clamp(20.0, 999.0);
        if self.engine.is_playing() {
            self.engine.set_bpm(self.project.tempo);
        }
        self.save();
        cx.notify();
    }

    pub fn current_step(&self) -> f64 {
        self.engine.current_step()
    }

    pub fn is_song_mode(&self) -> bool {
        self.engine.is_song_mode()
    }

    pub fn song_step(&self) -> f64 {
        self.engine.song_step()
    }

    /// Toggling Pattern/Song restarts playback in the new mode immediately
    /// if the transport is already running — mirrors the original's
    /// documented behavior rather than waiting for the next manual Play.
    pub fn toggle_song_mode(&mut self, cx: &mut Context<Self>) {
        let now_song = !self.engine.is_song_mode();
        self.engine.set_song_mode(now_song);
        if self.engine.is_playing() {
            self.engine.stop();
            self.engine.play(self.project.tempo);
        }
        cx.notify();
    }
}
