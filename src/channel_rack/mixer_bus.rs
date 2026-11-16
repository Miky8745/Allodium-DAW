//! Mixer-track bookkeeping the `Mixer` panel reads/writes through
//! `AppShell::channel_rack` — tracks live in the project model owned
//! here, not in `mixer.rs`, which only renders them.

use gpui::Context;

use crate::project::MixerTrack;

use super::ChannelRack;

impl ChannelRack {
    pub fn mixer_tracks(&self) -> &[MixerTrack] {
        &self.project.mixer_tracks.items
    }

    pub fn master_volume(&self) -> f64 {
        self.project.master_volume
    }

    /// Live update during a fader drag — rebuilds the mixer-bus snapshot
    /// so the change is audible immediately, but doesn't save; call
    /// `persist` at drag-end, same split as every other slider in this
    /// app.
    pub fn set_master_volume(&mut self, volume: f64, cx: &mut Context<Self>) {
        self.project.master_volume = volume.clamp(0.0, 1.0);
        self.push_snapshot();
        cx.notify();
    }

    pub fn add_mixer_track(&mut self, cx: &mut Context<Self>) {
        let id = self.next_mixer_track_id;
        self.next_mixer_track_id += 1;
        self.project.mixer_tracks.items.push(MixerTrack { id, volume: 0.8, pan: 0.5, muted: false, soloed: false, routes_to: 0 });
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    /// Any channel routed to the removed track falls back to Master,
    /// mirroring the original's "nothing goes silently unrouted" rule for
    /// a removed routing target.
    pub fn remove_mixer_track(&mut self, track_id: u32, cx: &mut Context<Self>) {
        self.project.mixer_tracks.items.retain(|t| t.id != track_id);
        for ch in &mut self.project.channels.items {
            if ch.mixer_track == track_id {
                ch.mixer_track = 0;
            }
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    pub fn set_mixer_track_volume(&mut self, track_id: u32, volume: f64, cx: &mut Context<Self>) {
        if let Some(t) = self.project.mixer_tracks.items.iter_mut().find(|t| t.id == track_id) {
            t.volume = volume.clamp(0.0, 1.0);
        }
        self.push_snapshot();
        cx.notify();
    }

    pub fn set_mixer_track_pan(&mut self, track_id: u32, pan: f64, cx: &mut Context<Self>) {
        if let Some(t) = self.project.mixer_tracks.items.iter_mut().find(|t| t.id == track_id) {
            t.pan = pan.clamp(0.0, 1.0);
        }
        self.push_snapshot();
        cx.notify();
    }

    pub fn toggle_mixer_mute(&mut self, track_id: u32, cx: &mut Context<Self>) {
        if let Some(t) = self.project.mixer_tracks.items.iter_mut().find(|t| t.id == track_id) {
            t.muted = !t.muted;
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    /// Multi-solo (unlike the Channel Rack's own single-solo) — per
    /// `docs/mixer.md`'s documented "standard multi-solo mixer semantics",
    /// a deliberately different rule from the Channel Rack's solo despite
    /// looking like the same button.
    pub fn toggle_mixer_solo(&mut self, track_id: u32, cx: &mut Context<Self>) {
        if let Some(t) = self.project.mixer_tracks.items.iter_mut().find(|t| t.id == track_id) {
            t.soloed = !t.soloed;
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }
}
