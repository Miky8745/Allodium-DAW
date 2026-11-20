//! Channel/pattern/placement bookkeeping that the Piano Roll and Playlist
//! read through `AppShell::channel_rack` — the "shared project state"
//! half of the rack, as opposed to `engine.rs`'s playback proxies or
//! `mixer_bus.rs`'s mixer-track bookkeeping.

use gpui::Context;

use crate::project::{self, Channel, Pattern, Placement};

use super::ChannelRack;

impl ChannelRack {
    pub fn channels(&self) -> &[Channel] {
        &self.project.channels.items
    }

    pub fn selected_channel_id(&self) -> Option<u32> {
        self.selected_channel_id
    }

    pub fn select_channel(&mut self, channel_id: u32, cx: &mut Context<Self>) {
        self.selected_channel_id = Some(channel_id);
        cx.notify();
    }

    /// Steps in the currently selected pattern, same formula `build_snapshot`
    /// uses for the scheduler, so the Piano Roll's grid width and the
    /// engine's loop point always agree.
    pub fn pattern_length_steps(&self) -> u32 {
        self.project
            .patterns
            .items
            .iter()
            .find(|p| p.id == self.project.selected_pattern_id)
            .map(|p| (p.length_beats * 4.0).round().max(1.0) as u32)
            .unwrap_or(16)
    }

    pub fn notes_for(&self, channel_id: u32) -> Vec<project::Note> {
        let pattern_id = self.project.selected_pattern_id;
        self.project
            .content
            .items
            .iter()
            .find(|e| e.pattern_id == pattern_id && e.channel_id == channel_id)
            .map(|e| e.notes.clone())
            .unwrap_or_default()
    }

    /// Every note across every channel for a given pattern, flattened —
    /// unlike `notes_for` (one channel, the *selected* pattern only), this
    /// takes an explicit `pattern_id` so a caller like the Playlist can
    /// preview any placed pattern's content, not just whichever one is
    /// currently open for editing.
    pub fn pattern_notes(&self, pattern_id: u32) -> Vec<project::Note> {
        self.project
            .content
            .items
            .iter()
            .filter(|e| e.pattern_id == pattern_id)
            .flat_map(|e| e.notes.iter().cloned())
            .collect()
    }

    /// Replaces a channel's notes and immediately feeds the engine a fresh
    /// snapshot so an edit is audible mid-playback, but does *not* write to
    /// disk — call `persist` for that, same split as a slider/dial drag
    /// uses, so a fast note-drag doesn't hit the filesystem every frame.
    pub fn update_notes(&mut self, channel_id: u32, notes: Vec<project::Note>, cx: &mut Context<Self>) {
        let entry = self.content_for(channel_id);
        entry.notes = notes;
        self.push_snapshot();
        cx.notify();
    }

    pub fn persist(&self) {
        self.save();
    }

    pub fn patterns(&self) -> &[Pattern] {
        &self.project.patterns.items
    }

    pub fn selected_pattern_id(&self) -> u32 {
        self.project.selected_pattern_id
    }

    /// Switches which pattern is being edited/drawn (Channel Rack, Piano
    /// Roll) *and* which pattern the Playlist draws when you click empty
    /// lane space — one "active pattern" concept shared by all three,
    /// mirroring the original's single `patternStore.selectedPatternId`.
    pub fn select_pattern(&mut self, pattern_id: u32, cx: &mut Context<Self>) {
        self.project.selected_pattern_id = pattern_id;
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    pub fn add_pattern(&mut self, cx: &mut Context<Self>) {
        let id = self.next_pattern_id;
        self.next_pattern_id += 1;
        self.project.patterns.items.push(Pattern { id, name: format!("Pattern {id}"), length_beats: 4.0 });
        self.project.selected_pattern_id = id;
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    pub fn beats_per_bar(&self) -> u32 {
        self.project.beats_per_bar
    }

    /// How many whole bars of the selected pattern actually hold a note,
    /// across every channel — the "content length" `sync_pattern_length`
    /// and `playback_length_steps` both measure against, as opposed to
    /// the pattern's own stored, editable `length_beats`. `0` means the
    /// pattern is entirely empty.
    fn used_bars(&self) -> u32 {
        let bar_steps = (self.beats_per_bar() as f64) * 4.0;
        let pattern_id = self.project.selected_pattern_id;
        let used_steps = self
            .project
            .content
            .items
            .iter()
            .filter(|e| e.pattern_id == pattern_id)
            .flat_map(|e| e.notes.iter())
            .map(|n| n.start + n.length)
            .fold(0.0_f64, f64::max);
        (used_steps / bar_steps).ceil() as u32
    }

    /// Keeps the selected pattern exactly one bar longer than its last
    /// used bar: grows it when a note reaches the current last bar, and
    /// shrinks it back down when trailing bars go empty again (e.g.
    /// after a delete) — so there's always exactly one spare empty bar
    /// at the end to sketch a variation into, never more. Call after any
    /// edit that can add or remove a note in the selected pattern.
    pub fn sync_pattern_length(&mut self, cx: &mut Context<Self>) {
        let bar_beats = self.beats_per_bar() as f64;
        let desired_beats = (self.used_bars() as f64 + 1.0) * bar_beats;
        let pattern_id = self.project.selected_pattern_id;
        let Some(p) = self.project.patterns.items.iter_mut().find(|p| p.id == pattern_id) else { return };
        if p.length_beats != desired_beats {
            p.length_beats = desired_beats;
            self.push_snapshot();
            self.save();
            cx.notify();
        }
    }

    /// Backs the header's time-signature scroll field — Allodium-tauri's
    /// own `ScrollField bind:value={channelStore.beatsPerBar} min={1}
    /// max={16}`.
    pub fn set_beats_per_bar(&mut self, value: u32, cx: &mut Context<Self>) {
        self.project.beats_per_bar = value.clamp(1, 16);
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    pub fn placements(&self) -> &[Placement] {
        &self.project.placements.items
    }

    /// Replaces the whole Playlist arrangement at once, like `update_notes`
    /// does for a channel's notes — the Playlist view owns its own local
    /// editing buffer and this is just the write-through.
    pub fn update_placements(&mut self, placements: Vec<Placement>, cx: &mut Context<Self>) {
        self.project.placements.items = placements;
        self.push_snapshot();
        cx.notify();
    }

    pub fn next_placement_id(&mut self) -> u32 {
        let id = self.next_placement_id;
        self.next_placement_id += 1;
        id
    }
}
