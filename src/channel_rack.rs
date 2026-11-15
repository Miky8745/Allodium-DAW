//! Channel Rack: a vertical list of channels, each with a sample slot, a
//! 16-step sequencer row, volume/pan sliders, and mute/solo — wired live
//! to the Stage 2 project model and the Stage 3/4 audio engine. See
//! `docs/stage-04-channel-rack.md`.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    ClickEvent, Context, IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement,
    Render, Styled, Window, div, prelude::*, px, rgb,
};

use crate::audio::{self, AudioEngine, SampleBuffer};
use crate::project::{self, Channel, MixerTrack, Pattern, PatternContentEntry, Placement, ProjectFile};
use crate::theme;

const STEP_COUNT: usize = 16;
const SLIDER_W: f32 = 64.0;
const PROJECT_NAME: &str = "default";

#[derive(Clone, Copy)]
enum SliderKind {
    Volume,
    Pan,
}

struct SliderDrag {
    channel_id: u32,
    kind: SliderKind,
    start_x: f32,
    start_value: f32,
}

pub struct ChannelRack {
    engine: Arc<AudioEngine>,
    project: ProjectFile,
    samples: HashMap<String, Arc<SampleBuffer>>,
    next_channel_id: u32,
    next_pattern_id: u32,
    next_placement_id: u32,
    next_mixer_track_id: u32,
    drag: Option<SliderDrag>,
    selected_channel_id: Option<u32>,
}

impl ChannelRack {
    pub fn new(engine: Arc<AudioEngine>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let project = project::load_project(PROJECT_NAME).ok().flatten().unwrap_or_default();

        let mut samples = HashMap::new();
        for ch in &project.channels.items {
            if ch.sample_path.is_empty() {
                continue;
            }
            match audio::decode_sample(std::path::Path::new(&ch.sample_path)) {
                Ok(buf) => {
                    samples.insert(ch.sample_path.clone(), buf);
                }
                Err(e) => eprintln!("[channel rack] couldn't load '{}': {e}", ch.sample_path),
            }
        }

        let next_channel_id = project.channels.items.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        let next_pattern_id = project.patterns.items.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let next_placement_id = project.placements.items.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let next_mixer_track_id = project.mixer_tracks.items.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        let selected_channel_id = project.channels.items.first().map(|c| c.id);

        let mut rack = Self {
            engine,
            project,
            samples,
            next_channel_id,
            next_pattern_id,
            next_placement_id,
            next_mixer_track_id,
            drag: None,
            selected_channel_id,
        };
        rack.push_snapshot();
        rack
    }

    pub fn is_playing(&self) -> bool {
        self.engine.is_playing()
    }

    pub fn tempo(&self) -> f64 {
        self.project.tempo
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

    /// Replaces a channel's notes and immediately feeds the engine a fresh
    /// snapshot so an edit is audible mid-playback, but does *not* write to
    /// disk — call `persist` for that, same split as the volume/pan slider
    /// drag uses, so a fast note-drag doesn't hit the filesystem every frame.
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

    /// Cycles a channel's mixer routing through Master → track 1 → track
    /// 2 → … → back to Master. The whole "pick a routing target" UI for
    /// the MVP, per `stages/07-mixer.md`'s single-target-only scope — no
    /// dropdown/menu needed for a handful of targets.
    pub fn cycle_channel_route(&mut self, channel_id: u32, cx: &mut Context<Self>) {
        let mut targets: Vec<u32> = vec![0];
        targets.extend(self.project.mixer_tracks.items.iter().map(|t| t.id));
        if let Some(ch) = self.project.channels.items.iter_mut().find(|c| c.id == channel_id) {
            let pos = targets.iter().position(|t| *t == ch.mixer_track).unwrap_or(0);
            ch.mixer_track = targets[(pos + 1) % targets.len()];
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    fn push_snapshot(&mut self) {
        match audio::build_snapshot(&self.project, &self.samples) {
            Ok(snapshot) => self.engine.set_content(snapshot),
            Err(e) => eprintln!("[channel rack] couldn't build snapshot: {e}"),
        }
        self.engine.set_song_content(audio::build_song_snapshot(&self.project, &self.samples));
        let (buses, master_volume) = audio::build_mixer_buses(&self.project);
        self.engine.set_mixer(buses, master_volume);
    }

    fn save(&self) {
        if let Err(e) = project::save_project(PROJECT_NAME, &self.project) {
            eprintln!("[channel rack] save failed: {e}");
        }
    }

    fn content_for(&mut self, channel_id: u32) -> &mut PatternContentEntry {
        let pattern_id = self.project.selected_pattern_id;
        if !self.project.content.items.iter().any(|e| e.pattern_id == pattern_id && e.channel_id == channel_id) {
            self.project.content.items.push(PatternContentEntry {
                pattern_id,
                channel_id,
                steps: vec![false; STEP_COUNT],
                notes: vec![],
            });
        }
        self.project
            .content
            .items
            .iter_mut()
            .find(|e| e.pattern_id == pattern_id && e.channel_id == channel_id)
            .unwrap()
    }

    fn audition(&self, channel_id: u32) {
        let Some(ch) = self.project.channels.items.iter().find(|c| c.id == channel_id) else { return };
        let Some(sample) = self.samples.get(&ch.sample_path) else { return };
        self.engine.preview(sample.clone(), ch.volume as f32, ch.pan as f32);
    }

    fn toggle_step(&mut self, channel_id: u32, step: usize, cx: &mut Context<Self>) {
        let entry = self.content_for(channel_id);
        if let Some(on) = entry.steps.get_mut(step) {
            *on = !*on;
        }
        self.audition(channel_id);
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    fn toggle_mute(&mut self, channel_id: u32, cx: &mut Context<Self>) {
        if let Some(ch) = self.project.channels.items.iter_mut().find(|c| c.id == channel_id) {
            ch.muted = !ch.muted;
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    /// Single-solo, matching the original: soloing one channel clears
    /// every other channel's solo instead of allowing several at once.
    fn toggle_solo(&mut self, channel_id: u32, cx: &mut Context<Self>) {
        let now_soloed = self
            .project
            .channels
            .items
            .iter()
            .find(|c| c.id == channel_id)
            .map(|c| !c.soloed)
            .unwrap_or(false);
        for ch in &mut self.project.channels.items {
            ch.soloed = now_soloed && ch.id == channel_id;
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    fn remove_channel(&mut self, channel_id: u32, cx: &mut Context<Self>) {
        self.project.channels.items.retain(|c| c.id != channel_id);
        self.project.content.items.retain(|e| e.channel_id != channel_id);
        if self.selected_channel_id == Some(channel_id) {
            self.selected_channel_id = self.project.channels.items.first().map(|c| c.id);
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    fn add_channel(&mut self, cx: &mut Context<Self>) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("audio", &["wav", "ogg", "mp3", "flac"])
            .pick_file()
        else {
            return;
        };
        let path_str = path.to_string_lossy().to_string();
        let sample = match audio::decode_sample(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[channel rack] couldn't decode {path_str}: {e}");
                return;
            }
        };
        self.samples.insert(path_str.clone(), sample);

        let id = self.next_channel_id;
        self.next_channel_id += 1;
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Channel".to_string());
        self.project.channels.items.push(Channel {
            id,
            name,
            sample_path: path_str,
            muted: false,
            soloed: false,
            pan: 0.5,
            volume: 0.8,
            mixer_track: 0,
        });
        let pattern_id = self.project.selected_pattern_id;
        self.project.content.items.push(PatternContentEntry {
            pattern_id,
            channel_id: id,
            steps: vec![false; STEP_COUNT],
            notes: vec![],
        });

        self.selected_channel_id = Some(id);
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    pub fn toggle_play(&mut self, cx: &mut Context<Self>) {
        if self.engine.is_playing() {
            self.engine.stop();
        } else {
            self.engine.play(self.project.tempo);
        }
        cx.notify();
    }

    fn begin_slider_drag(&mut self, channel_id: u32, kind: SliderKind, start_x: f32) {
        let start_value = match (kind, self.project.channels.items.iter().find(|c| c.id == channel_id)) {
            (SliderKind::Volume, Some(ch)) => ch.volume as f32,
            (SliderKind::Pan, Some(ch)) => ch.pan as f32,
            _ => 0.0,
        };
        self.drag = Some(SliderDrag { channel_id, kind, start_x, start_value });
    }

    fn update_slider_drag(&mut self, current_x: f32, cx: &mut Context<Self>) {
        let Some(drag) = &self.drag else { return };
        let (channel_id, kind, start_x, start_value) = (drag.channel_id, drag.kind, drag.start_x, drag.start_value);
        let delta = (current_x - start_x) / SLIDER_W;
        let new_value = (start_value + delta).clamp(0.0, 1.0) as f64;
        if let Some(ch) = self.project.channels.items.iter_mut().find(|c| c.id == channel_id) {
            match kind {
                SliderKind::Volume => ch.volume = new_value,
                SliderKind::Pan => ch.pan = new_value,
            }
        }
        self.push_snapshot();
        cx.notify();
    }

    fn end_slider_drag(&mut self) {
        if self.drag.take().is_some() {
            self.save();
        }
    }
}

fn slider(label: &'static str, value: f64, channel_id: u32, kind: SliderKind, cx: &Context<ChannelRack>) -> impl IntoElement {
    let fill_w = (value.clamp(0.0, 1.0) as f32) * SLIDER_W;
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(div().text_color(theme::ACCENT_DIM).text_size(px(10.0)).child(label))
        .child(
            div()
                .w(px(SLIDER_W))
                .h(px(10.0))
                .rounded(px(5.0))
                .bg(theme::BG_PANEL)
                .relative()
                .child(div().absolute().left_0().top_0().h_full().w(px(fill_w)).rounded(px(5.0)).bg(theme::ACCENT))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, ev: &gpui::MouseDownEvent, _, _| {
                        this.begin_slider_drag(channel_id, kind, f32::from(ev.position.x));
                    }),
                ),
        )
}

impl Render for ChannelRack {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.request_animation_frame();
        cx.notify();

        let playing = self.engine.is_playing();
        let song_mode = self.engine.is_song_mode();
        let current_step = if playing { self.engine.current_step().floor() as i64 } else { -1 };
        let channel_ids: Vec<u32> = self.project.channels.items.iter().map(|c| c.id).collect();
        let pattern_id = self.project.selected_pattern_id;

        let mut rows = div().flex().flex_col().gap_2().p_3();
        for id in channel_ids {
            let ch = self.project.channels.items.iter().find(|c| c.id == id).unwrap().clone();
            let steps = self
                .project
                .content
                .items
                .iter()
                .find(|e| e.pattern_id == pattern_id && e.channel_id == id)
                .map(|e| e.steps.clone())
                .unwrap_or_else(|| vec![false; STEP_COUNT]);

            let mut step_row = div().flex().flex_row().gap_1();
            for (i, on) in steps.iter().enumerate().take(STEP_COUNT) {
                let is_current = playing && current_step == i as i64;
                // 4-step beat-group banding, matching the Piano Roll's
                // own beat-group shading — the two grids should read as
                // one system, not two unrelated ones.
                let in_even_group = (i / 4) % 2 == 0;
                let bg = match (*on, in_even_group) {
                    (true, _) => theme::ACCENT,
                    (false, true) => theme::BG_PANEL,
                    (false, false) => theme::BG_PILL,
                };
                step_row = step_row.child(
                    div()
                        .id(("step", (id as usize) * 100 + i))
                        .w(px(20.0))
                        .h(px(20.0))
                        .rounded(px(4.0))
                        .bg(bg)
                        .when(is_current, |d| d.border_2().border_color(theme::ACCENT_TEXT))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.toggle_step(id, i, cx);
                        })),
                );
            }

            let name_label = if ch.name.is_empty() { "Drop sample".to_string() } else { ch.name.clone() };
            let is_selected = self.selected_channel_id == Some(id);
            let row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .p_2()
                .rounded(theme::RADIUS_SM)
                .bg(theme::BG_PILL)
                .when(is_selected, |d| d.border_1().border_color(theme::ACCENT))
                .child(
                    div()
                        .id(("name", id as usize))
                        .w(px(140.0))
                        .cursor_pointer()
                        .child(name_label)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.audition(id);
                            this.select_channel(id, cx);
                        })),
                )
                .child(
                    div()
                        .id(("mute", id as usize))
                        .w(px(20.0))
                        .h(px(20.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(10.0))
                        .bg(if ch.muted { theme::MUTE_RED } else { theme::BG_PANEL })
                        .cursor_pointer()
                        .child("M")
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_mute(id, cx))),
                )
                .child(
                    div()
                        .id(("solo", id as usize))
                        .w(px(28.0))
                        .h(px(20.0))
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(10.0))
                        .bg(if ch.soloed { theme::ACCENT } else { theme::BG_PANEL })
                        .cursor_pointer()
                        .child("S")
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_solo(id, cx))),
                )
                .child(
                    div()
                        .id(("route", id as usize))
                        .w(px(44.0))
                        .h(px(20.0))
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(9.0))
                        .bg(theme::BG_PANEL)
                        .text_color(theme::ACCENT_DIM)
                        .cursor_pointer()
                        .child(if ch.mixer_track == 0 { "Mstr".to_string() } else { format!("Trk {}", ch.mixer_track) })
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.cycle_channel_route(id, cx))),
                )
                .child(slider("VOL", ch.volume, id, SliderKind::Volume, cx))
                .child(slider("PAN", ch.pan, id, SliderKind::Pan, cx))
                .child(step_row)
                .child(
                    div()
                        .id(("remove", id as usize))
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(theme::MUTE_RED)
                        .cursor_pointer()
                        .child("x")
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.remove_channel(id, cx))),
                );
            rows = rows.child(row);
        }

        let transport = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .p_2()
            .child(
                div()
                    .id("play-stop")
                    .w(px(72.0))
                    .h(px(24.0))
                    .rounded(theme::RADIUS_SM)
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .bg(if playing { theme::ACCENT } else { theme::BG_PILL })
                    .cursor_pointer()
                    .child(if playing { "■" } else { "▶" })
                    .child(if playing { "Stop" } else { "Play" })
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_play(cx))),
            )
            .child(
                div()
                    .id("transport-mode")
                    .w(px(52.0))
                    .h(px(24.0))
                    .rounded(theme::RADIUS_SM)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.0))
                    .bg(if song_mode { theme::ACCENT } else { theme::BG_PILL })
                    .cursor_pointer()
                    .child(if song_mode { "Song" } else { "Pat" })
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_song_mode(cx))),
            )
            .child(
                div()
                    .id("add-channel")
                    .h(px(24.0))
                    .px_2()
                    .rounded(theme::RADIUS_SM)
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme::BG_PILL)
                    .cursor_pointer()
                    .child("+ Channel")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add_channel(cx))),
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .text_color(rgb(0xe9edf4))
            .font_family(theme::FONT_UI)
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if this.drag.is_some() {
                    this.update_slider_drag(f32::from(ev.position.x), cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, _| this.end_slider_drag()),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, _| this.end_slider_drag()),
            )
            .child(transport)
            .child(div().flex_1().overflow_hidden().child(rows))
    }
}
