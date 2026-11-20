//! Playlist / arrangement: a fixed set of tracks, a horizontal timeline,
//! place/move/delete pattern blocks, Song-mode playback. See
//! `docs/stage-06-playlist.md`.
//!
//! No clip (audio-file) placements for the MVP — patterns only, per
//! `../daw-rewrite-starter/stages/06-playlist.md`.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use gpui::{
    Context, Entity, Hsla, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, Render, ScrollEvent, Styled, Window, canvas, div, point,
    prelude::*, px, quad, rgb, size,
};

use crate::audio::SampleBuffer;
use crate::channel_rack::ChannelRack;
use crate::file_browser::SampleDragPayload;
use crate::project::{Note, Pattern, Placement};
use crate::theme;

/// A block's title text: a clip's own sample filename, or the referenced
/// pattern's name. Shared by the live block layer and the dying-block
/// fade-out layer so both agree on what a placement is called.
fn block_label(placement: &Placement, patterns: &[Pattern]) -> String {
    if !placement.sample_path.is_empty() {
        std::path::Path::new(&placement.sample_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Sample".to_string())
    } else {
        patterns.iter().find(|p| p.id == placement.pattern_id).map(|p| p.name.clone()).unwrap_or_default()
    }
}

/// The content area's background — distinct from the grid's own dark fill
/// so each block's "what's inside it" area reads as its own surface, not
/// a continuation of the grid behind it.
const CONTENT_BG: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.14, a: 1.0 };
/// Height of a block's title strip; the rest of `ROW_H` is the content
/// area below it.
const TITLE_H: f32 = 14.0;

/// Not persisted — a fixed visual lane count, same as the original's own
/// 8 hardcoded tracks (`docs/playlist.md`: "local `$state`... not a
/// store, since there's no per-track behavior yet beyond visual lanes").
const TRACK_COUNT: u32 = 8;
const BAR_COUNT: u32 = 64;
const BEAT_W: f32 = 40.0;
const ROW_H: f32 = 40.0;
const TRACK_PANEL_W: f32 = 76.0;
/// The timebar above the grid, numbering each bar.
const RULER_H: f32 = 20.0;
/// How many bars share one alternating background color.
const BARS_PER_GROUP: u32 = 4;
const DEATH_FADE_SECS: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq)]
enum DragMode {
    None,
    /// Dragging a placement (freshly placed or pre-existing) by its body.
    /// `grab_offset_beats`/`grab_offset_track` record where inside the
    /// block the initial click landed, mirroring the Piano Roll's note
    /// drag — same shape of problem, duplicated rather than shared since
    /// the stage brief explicitly invites that over a premature
    /// abstraction.
    Moving { placement_id: u32, grab_offset_beats: f64, grab_offset_track: i32 },
    Deleting,
}

struct DyingPlacement {
    placement: Placement,
    pattern_name: String,
    died_at: Instant,
}

pub struct Playlist {
    channel_rack: Entity<ChannelRack>,
    placements: Vec<Placement>,
    scroll_x: f32,
    scroll_y: f32,
    drag: DragMode,
    dying: Vec<DyingPlacement>,
    grid_origin: Rc<Cell<Point<Pixels>>>,
    /// The grid canvas's own on-screen size, captured alongside
    /// `grid_origin`. `clamp_scroll` needs the visible grid area's height,
    /// not `window.viewport_size()` (the whole OS window, always taller
    /// than this floating panel's own grid area) - see `piano_roll.rs`'s
    /// identical fix for the bug this caused (scroll stopping short of
    /// the real content end).
    grid_size: Rc<Cell<gpui::Size<Pixels>>>,
}

impl Playlist {
    pub fn new(channel_rack: Entity<ChannelRack>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placements = channel_rack.read(cx).placements().to_vec();
        Self {
            channel_rack,
            placements,
            scroll_x: 0.0,
            scroll_y: 0.0,
            drag: DragMode::None,
            dying: Vec::new(),
            grid_origin: Rc::new(Cell::new(point(px(0.0), px(0.0)))),
            grid_size: Rc::new(Cell::new(size(px(0.0), px(0.0)))),
        }
    }

    /// A pattern's displayed/placed length in the Playlist: its content
    /// (every channel's notes), rounded up to a whole bar — never the
    /// trailing spare bar `ChannelRack::sync_pattern_length` always pads
    /// `length_beats` with for editing. Everything that cares how long a
    /// placed pattern is (block width/paint, overlap checks, drag hit-
    /// testing) reads this instead of the pattern's raw stored length,
    /// so a placement is never drawn, or collides, wider than what it
    /// actually plays.
    fn pattern_length_beats(&self, pattern_id: u32, cx: &Context<Self>) -> f64 {
        let rack = self.channel_rack.read(cx);
        let bar_beats = rack.beats_per_bar().max(1) as f64;
        let bar_steps = bar_beats * 4.0;
        let used_steps = rack.pattern_notes(pattern_id).iter().map(|n| n.start + n.length).fold(0.0_f64, f64::max);
        let used_bars = (used_steps / bar_steps).ceil();
        used_bars.max(1.0) * bar_beats
    }

    fn content_width(&self, cx: &Context<Self>) -> f32 {
        let beats_per_bar = self.channel_rack.read(cx).beats_per_bar().max(1);
        (BAR_COUNT * beats_per_bar) as f32 * BEAT_W
    }

    fn content_height(&self) -> f32 {
        TRACK_COUNT as f32 * ROW_H
    }

    fn clamp_scroll(&mut self, viewport: gpui::Size<gpui::Pixels>, cx: &Context<Self>) {
        let max_x = (self.content_width(cx) - f32::from(viewport.width)).max(0.0);
        let max_y = (self.content_height() - f32::from(viewport.height)).max(0.0);
        self.scroll_x = self.scroll_x.clamp(0.0, max_x);
        self.scroll_y = self.scroll_y.clamp(0.0, max_y);
    }

    fn placement_at(&self, beat: f64, track: i32, cx: &Context<Self>) -> Option<usize> {
        self.placements.iter().position(|p| {
            if p.track_id as i32 != track {
                return false;
            }
            let len = self.pattern_length_beats(p.pattern_id, cx);
            beat >= p.start_beat && beat < p.start_beat + len
        })
    }

    /// True if placing a pattern of `length` beats at `(start_beat, track)`
    /// would overlap any existing placement other than `ignore_id`. The
    /// stage brief's explicitly-chosen MVP simplification over the
    /// original's `nearestFreeStart()` "snap to nearest free slot".
    fn overlaps(&self, start_beat: f64, length: f64, track: u32, ignore_id: Option<u32>, cx: &Context<Self>) -> bool {
        self.placements.iter().any(|p| {
            if p.track_id != track || Some(p.id) == ignore_id {
                return false;
            }
            let other_len = self.pattern_length_beats(p.pattern_id, cx);
            start_beat < p.start_beat + other_len && start_beat + length > p.start_beat
        })
    }

    fn pointer_to_grid(&self, position: Point<gpui::Pixels>) -> (f64, i32) {
        let origin = self.grid_origin.get();
        let x = f32::from(position.x - origin.x) + self.scroll_x;
        let y = f32::from(position.y - origin.y) + self.scroll_y;
        let beat = (x / BEAT_W).floor().max(0.0) as f64;
        let track = (y / ROW_H).floor() as i32;
        (beat, track)
    }

    fn handle_mouse_down(&mut self, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        let (beat, track) = self.pointer_to_grid(ev.position);
        if track < 0 || track as u32 >= TRACK_COUNT {
            return;
        }

        match ev.button {
            MouseButton::Right => {
                self.drag = DragMode::Deleting;
                self.delete_at(beat, track, cx);
            }
            MouseButton::Left => {
                if let Some(idx) = self.placement_at(beat, track, cx) {
                    let placement = self.placements[idx].clone();
                    self.drag = DragMode::Moving {
                        placement_id: placement.id,
                        grab_offset_beats: beat - placement.start_beat,
                        grab_offset_track: track - placement.track_id as i32,
                    };
                } else {
                    let pattern_id = self.channel_rack.read(cx).selected_pattern_id();
                    let length = self.pattern_length_beats(pattern_id, cx);
                    if self.overlaps(beat, length, track as u32, None, cx) {
                        return;
                    }
                    let id = self.channel_rack.update(cx, |rack, _| rack.next_placement_id());
                    self.placements.push(Placement {
                        id,
                        pattern_id,
                        track_id: track as u32,
                        start_beat: beat,
                        sample_path: String::new(),
                        length_beats: 0.0,
                    });
                    self.drag = DragMode::Moving { placement_id: id, grab_offset_beats: 0.0, grab_offset_track: 0 };
                }
            }
            _ => return,
        }
        self.commit_live(cx);
    }

    fn handle_mouse_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let (beat, track) = self.pointer_to_grid(ev.position);

        match self.drag {
            DragMode::Moving { placement_id, grab_offset_beats, grab_offset_track } => {
                let Some(idx) = self.placements.iter().position(|p| p.id == placement_id) else { return };
                let length = self.pattern_length_beats(self.placements[idx].pattern_id, cx);
                let new_start = (beat - grab_offset_beats).max(0.0);
                let new_track = (track - grab_offset_track).clamp(0, TRACK_COUNT as i32 - 1) as u32;
                if !self.overlaps(new_start, length, new_track, Some(placement_id), cx) {
                    let p = &mut self.placements[idx];
                    p.start_beat = new_start;
                    p.track_id = new_track;
                    self.commit_live(cx);
                }
            }
            DragMode::Deleting => {
                if track >= 0 && (track as u32) < TRACK_COUNT {
                    self.delete_at(beat, track, cx);
                    self.commit_live(cx);
                }
            }
            DragMode::None => {}
        }
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag != DragMode::None {
            self.drag = DragMode::None;
            self.channel_rack.update(cx, |rack, _| rack.persist());
        }
    }

    fn delete_at(&mut self, beat: f64, track: i32, cx: &mut Context<Self>) {
        if let Some(idx) = self.placement_at(beat, track, cx) {
            let placement = self.placements.remove(idx);
            let pattern_name = block_label(&placement, self.channel_rack.read(cx).patterns());
            self.dying.push(DyingPlacement { placement, pattern_name, died_at: Instant::now() });
        }
    }

    /// Drag-a-sample-straight-from-the-File-Browser-onto-the-Playlist —
    /// creates a one-shot "clip" placement (`Placement.sample_path` non-
    /// empty, `pattern_id` 0) the same way clicking empty grid space
    /// creates a pattern placement, just with its length computed from
    /// the sample's own real duration (`frame_count / sample_rate`
    /// seconds, converted to beats at the project's current tempo)
    /// instead of looked up from a pattern. Kept as the exact continuous
    /// value, *not* snapped to a whole beat — the block should be exactly
    /// as long as the sample actually plays for at this tempo, not fitted
    /// to the grid (only the clip's *start* snaps to a whole beat, same
    /// as every other placement).
    fn handle_sample_drop(&mut self, payload: &SampleDragPayload, position: Point<gpui::Pixels>, cx: &mut Context<Self>) {
        let (beat, track) = self.pointer_to_grid(position);
        if track < 0 || track as u32 >= TRACK_COUNT {
            return;
        }
        let path = std::path::PathBuf::from(&payload.path);
        let sample = match self.channel_rack.update(cx, |rack, _| rack.cache_sample(&path)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[playlist] couldn't decode '{}': {e}", payload.path);
                return;
            }
        };
        let tempo = self.channel_rack.read(cx).tempo();
        let duration_secs = sample.frame_count() as f64 / sample.sample_rate as f64;
        let length_beats = (duration_secs * tempo / 60.0).max(0.01);
        if self.overlaps(beat, length_beats, track as u32, None, cx) {
            return;
        }
        let id = self.channel_rack.update(cx, |rack, _| rack.next_placement_id());
        self.placements.push(Placement {
            id,
            pattern_id: 0,
            track_id: track as u32,
            start_beat: beat,
            sample_path: payload.path.clone(),
            length_beats,
        });
        self.commit_live(cx);
        self.channel_rack.update(cx, |rack, _| rack.persist());
    }

    /// Playlist ruler click-to-seek — meaningless in pattern mode (per
    /// `docs/audio-playback.md`'s "Song start marker": "the timeline
    /// position has no relationship to what's playing" outside Song
    /// mode), so it's a no-op there rather than silently seeking a
    /// transport the ruler doesn't represent.
    fn handle_ruler_click(&mut self, position: Point<gpui::Pixels>, cx: &mut Context<Self>) {
        if !self.channel_rack.read(cx).is_song_mode() {
            return;
        }
        let x = f32::from(position.x - self.grid_origin.get().x) + self.scroll_x;
        let beat = (x / BEAT_W).max(0.0) as f64;
        self.channel_rack.update(cx, |rack, _| rack.seek_song_to_step(beat * 4.0));
        cx.notify();
    }

    fn prune_dying(&mut self) {
        self.dying.retain(|d| d.died_at.elapsed().as_secs_f32() < DEATH_FADE_SECS);
    }

    fn commit_live(&mut self, cx: &mut Context<Self>) {
        let placements = self.placements.clone();
        self.channel_rack.update(cx, |rack, cx| rack.update_placements(placements, cx));
    }
}

impl Render for Playlist {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.clamp_scroll(self.grid_size.get(), cx);
        self.prune_dying();
        window.request_animation_frame();

        let rack = self.channel_rack.read(cx);
        let song_mode = rack.is_song_mode();
        let playing = rack.is_playing();
        let song_step = if song_mode && playing { rack.song_step() } else { -1.0 };
        let beats_per_bar = rack.beats_per_bar().max(1);
        let patterns: Vec<(u32, String, f64)> =
            rack.patterns().iter().map(|p| (p.id, p.name.clone(), self.pattern_length_beats(p.id, cx))).collect();

        let pattern_list = rack.patterns();
        let blocks: Vec<(Placement, String)> =
            self.placements.iter().map(|p| (p.clone(), block_label(p, pattern_list))).collect();
        let dying: Vec<(Placement, String, f32)> = self
            .dying
            .iter()
            .map(|d| (d.placement.clone(), d.pattern_name.clone(), d.died_at.elapsed().as_secs_f32() / DEATH_FADE_SECS))
            .collect();

        let scroll_x = self.scroll_x;
        let scroll_y = self.scroll_y;
        let grid_origin = self.grid_origin.clone();
        let grid_size = self.grid_size.clone();

        let mut track_labels = div().flex().flex_col().w(px(TRACK_PANEL_W)).h_full().bg(theme::BG_PANEL);
        for i in 0..TRACK_COUNT {
            track_labels = track_labels.child(
                div()
                    .h(px(ROW_H))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(10.0))
                    .text_color(theme::accent_dim())
                    .border_b_1()
                    .border_color(theme::BORDER)
                    .child(format!("Track {}", i + 1)),
            );
        }

        let mut block_layer = div().absolute().left_0().top_0().size_full();
        for (placement, name) in &blocks {
            let is_clip = !placement.sample_path.is_empty();
            let length = if is_clip {
                placement.length_beats.max(0.0)
            } else {
                patterns.iter().find(|(id, ..)| *id == placement.pattern_id).map(|(_, _, l)| *l).unwrap_or(4.0)
            };
            let left = placement.start_beat as f32 * BEAT_W - scroll_x;
            let top = placement.track_id as f32 * ROW_H - scroll_y;
            let width = (length as f32 * BEAT_W - 2.0).max(1.0);

            // What's actually inside this block: a clip shows its own
            // waveform; a pattern shows its aggregated note content as a
            // simplified, non-interactive mini-MIDI preview; an empty
            // pattern shows just the bare content background.
            let content: gpui::AnyElement = if is_clip {
                match rack.sample_for(&placement.sample_path) {
                    Some(sample) => canvas(move |_, _, _| {}, move |bounds, _, window, _| paint_waveform(window, bounds, &sample))
                        .size_full()
                        .into_any_element(),
                    None => div().size_full().into_any_element(),
                }
            } else {
                let notes = rack.pattern_notes(placement.pattern_id);
                if notes.is_empty() {
                    div().size_full().into_any_element()
                } else {
                    canvas(move |_, _, _| {}, move |bounds, _, window, _| paint_mini_notes(window, bounds, &notes, length))
                        .size_full()
                        .into_any_element()
                }
            };

            block_layer = block_layer.child(
                div()
                    .absolute()
                    .left(px(left + 1.0))
                    .top(px(top + 1.0))
                    .w(px(width))
                    .h(px(ROW_H - 2.0))
                    .rounded(theme::RADIUS_SM)
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(TITLE_H))
                            .w_full()
                            .flex_shrink_0()
                            .bg(theme::pattern_color(placement.pattern_id))
                            .flex()
                            .items_center()
                            .px_1()
                            .overflow_hidden()
                            .text_size(px(9.0))
                            .text_color(theme::accent_text())
                            .child(name.clone()),
                    )
                    .child(div().flex_1().w_full().bg(CONTENT_BG).child(content)),
            );
        }
        for (placement, name, progress) in &dying {
            let length = if !placement.sample_path.is_empty() {
                placement.length_beats.max(0.0)
            } else {
                patterns.iter().find(|(id, ..)| *id == placement.pattern_id).map(|(_, _, l)| *l).unwrap_or(4.0)
            };
            let left = placement.start_beat as f32 * BEAT_W - scroll_x;
            let top = placement.track_id as f32 * ROW_H - scroll_y;
            let width = (length as f32 * BEAT_W - 2.0).max(1.0);
            let alpha = (1.0 - progress).max(0.0);
            let mut dying_color = theme::warm_accent();
            dying_color.a = alpha;
            block_layer = block_layer.child(
                div()
                    .absolute()
                    .left(px(left + 1.0))
                    .top(px(top + 1.0))
                    .w(px(width))
                    .h(px(ROW_H - 2.0))
                    .rounded(theme::RADIUS_SM)
                    .bg(dying_color)
                    .flex()
                    .items_center()
                    .px_1()
                    .overflow_hidden()
                    .text_size(px(10.0))
                    .text_color(theme::accent_text())
                    .child(name.clone()),
            );
        }

        let mut pattern_bar = div().flex().flex_row().gap_1().px_2().items_center();
        for (id, name, _) in &patterns {
            let is_selected = *id == rack.selected_pattern_id();
            let id = *id;
            pattern_bar = pattern_bar.child(
                div()
                    .id(("pattern", id as usize))
                    .px_2()
                    .h(px(20.0))
                    .rounded(theme::RADIUS_SM)
                    .flex()
                    .items_center()
                    .text_size(px(10.0))
                    .bg(if is_selected { theme::pattern_color(id) } else { theme::BG_PILL })
                    .text_color(if is_selected { theme::accent_text() } else { theme::accent_dim() })
                    .cursor_pointer()
                    .when(!is_selected, |d| d.hover(|s| s.bg(theme::BG_PILL_HOVER)))
                    .child(name.clone())
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                        this.channel_rack.update(cx, |rack, cx| rack.select_pattern(id, cx));
                    })),
            );
        }
        pattern_bar = pattern_bar.child(
            div()
                .id("add-pattern")
                .px_2()
                .h(px(20.0))
                .rounded(theme::RADIUS_SM)
                .flex()
                .items_center()
                .text_size(px(10.0))
                .bg(theme::BG_PILL)
                .cursor_pointer()
                .hover(|s| s.bg(theme::BG_PILL_HOVER))
                .active(|s| s.bg(theme::BG_PILL_ACTIVE))
                .child("+ Pattern")
                .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.channel_rack.update(cx, |rack, cx| rack.add_pattern(cx));
                })),
        );

        // Numbers the grid's bold bar gridlines (see `paint_lanes`) — only
        // the bars actually in view, mirroring how `paint_lanes` itself
        // only paints the visible bar/beat range rather than the whole
        // 64-bar timeline.
        let bar_w = beats_per_bar as f32 * BEAT_W;
        let viewport_w = f32::from(self.grid_size.get().width).max(0.0);
        let first_bar_label = (scroll_x / bar_w).floor().max(0.0) as i32;
        let last_bar_label = ((scroll_x + viewport_w) / bar_w).ceil() as i32;
        let mut ruler_labels = div().absolute().left_0().top_0().size_full();
        for bar in first_bar_label..=last_bar_label {
            let x = bar as f32 * bar_w - scroll_x;
            ruler_labels = ruler_labels.child(
                div()
                    .absolute()
                    .left(px(x + 3.0))
                    .top_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .text_size(px(9.0))
                    .text_color(theme::accent_dim())
                    .child(format!("{}", bar + 1)),
            );
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::BG_APP)
            .font_family(theme::FONT_UI)
            .child(
                div()
                    .h(px(24.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .bg(theme::BG_PANEL)
                    .border_b_1()
                    .border_color(theme::BORDER)
                    .text_color(theme::accent_dim())
                    .child(pattern_bar),
            )
            .child(
                div()
                    .h(px(RULER_H))
                    .w_full()
                    .flex()
                    .flex_row()
                    .bg(theme::BG_PANEL)
                    .border_b_1()
                    .border_color(theme::BORDER)
                    .child(div().w(px(TRACK_PANEL_W)).h_full())
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .relative()
                            .overflow_hidden()
                            .cursor_pointer()
                            .child(ruler_labels)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                                    this.handle_ruler_click(ev.position, cx)
                                }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .flex_row()
                    .child(track_labels)
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .relative()
                            .overflow_hidden()
                            .bg(rgb(0x1a1a1a))
                            .on_scroll(cx.listener(|this, ev: &ScrollEvent, _window, cx| {
                                let delta = ev.delta.pixel_delta(px(ROW_H));
                                this.scroll_x -= f32::from(delta.x);
                                this.scroll_y -= f32::from(delta.y);
                                this.clamp_scroll(this.grid_size.get(), cx);
                                cx.notify();
                            }))
                            .child(
                                canvas(
                                    move |_, _, _| {},
                                    move |bounds, _, window, _| {
                                        grid_origin.set(bounds.origin);
                                        grid_size.set(bounds.size);
                                        paint_lanes(window, bounds, scroll_x, scroll_y, beats_per_bar);
                                    },
                                )
                                .size_full(),
                            )
                            .child(block_layer)
                            .child(
                                // `.absolute()` is load-bearing: this
                                // parent div isn't a flex container, so a
                                // plain `.size_full()` sibling here would
                                // stack in normal block flow *below* the
                                // grid canvas above (which only looks like
                                // an overlay because it's the sole child
                                // before `block_layer`'s own `.absolute()`
                                // takes it out of flow) — pushed below the
                                // visible, clipped area entirely instead of
                                // overlaying on top of it.
                                div().absolute().left_0().top_0().size_full().child(
                                    canvas(move |_, _, _| {}, move |bounds, _, window, _| paint_playhead(window, bounds, scroll_x, song_step))
                                        .size_full(),
                                ),
                            )
                            // Drag a sample straight from the File Browser
                            // onto the timeline — reuses the same payload
                            // type Channel Rack rows/footer already drop
                            // targets for (`SampleDragPayload`), the other
                            // way a sample reaches this app.
                            .drag_over::<SampleDragPayload>(|style, _, _, _| {
                                style.border_1().border_color(theme::accent()).bg(theme::accent().alpha(0.1))
                            })
                            .on_drop(cx.listener(|this, payload: &SampleDragPayload, window, cx| {
                                let position = window.mouse_position();
                                this.handle_sample_drop(payload, position, cx);
                            }))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, ev: &MouseDownEvent, _, cx| this.handle_mouse_down(ev, cx)),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(|this, ev: &MouseDownEvent, _, cx| this.handle_mouse_down(ev, cx)),
                            )
                            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                                if this.drag != DragMode::None {
                                    this.handle_mouse_move(ev, cx);
                                }
                            }))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                                    this.end_drag(cx);
                                    cx.notify();
                                }),
                            )
                            .on_mouse_up(
                                MouseButton::Right,
                                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                                    this.end_drag(cx);
                                    cx.notify();
                                }),
                            )
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                                    this.end_drag(cx);
                                    cx.notify();
                                }),
                            ),
                    ),
            )
    }
}

/// Vertical bar-group striping + beat/bar gridlines + thin row separators.
/// Placement blocks themselves are plain `div()`s layered on top (see
/// `render`) rather than hand-painted, unlike the Piano Roll — there are
/// only ever a handful of them, so GPUI's own text layout for
/// pattern-name labels is simpler than hand-shaping text into a `canvas`.
/// The ruler above the grid (also built in `render`) numbers the same
/// bold bar lines this paints. The Song-mode playhead is `paint_playhead`,
/// a separate canvas painted after the placement blocks instead of here —
/// see its own doc comment for why.
fn paint_lanes(window: &mut Window, bounds: gpui::Bounds<gpui::Pixels>, scroll_x: f32, scroll_y: f32, beats_per_bar: u32) {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let origin = bounds.origin;

    // The grid's alternating color is per *group of bars*, not per row or
    // single bar — each group spans the grid's full height.
    let bar_w = beats_per_bar as f32 * BEAT_W;
    let group_w = BARS_PER_GROUP as f32 * bar_w;
    let first_group = (scroll_x / group_w).floor().max(0.0) as i32;
    let last_group = ((scroll_x + width) / group_w).ceil() as i32;
    for group in first_group..=last_group {
        if group % 2 != 0 {
            continue;
        }
        let x = origin.x + px(group as f32 * group_w - scroll_x);
        window.paint_quad(quad(
            gpui::Bounds { origin: point(x, origin.y), size: size(px(group_w), px(height)) },
            px(0.0),
            Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.03 },
            px(0.0),
            gpui::transparent_black(),
            Default::default(),
        ));
    }

    // Thin row separators replace the old per-row tint now that coloring
    // runs vertically — tracks still need some way to read apart.
    for row in 1..TRACK_COUNT {
        let y = origin.y + px(row as f32 * ROW_H - scroll_y);
        window.paint_quad(quad(
            gpui::Bounds { origin: point(origin.x, y), size: size(px(width), px(1.0)) },
            px(0.0),
            Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.05 },
            px(0.0),
            gpui::transparent_black(),
            Default::default(),
        ));
    }

    // Beat lines inside each bar — plain, lighter lines. Skips multiples of
    // `beats_per_bar` (the bar's own start), which get the bold bar line
    // below instead.
    let first_beat = (scroll_x / BEAT_W).floor().max(0.0) as i64;
    let last_beat = ((scroll_x + width) / BEAT_W).ceil() as i64;
    for beat in first_beat..=last_beat {
        if beat.rem_euclid(beats_per_bar as i64) == 0 {
            continue; // a bar boundary — drawn bold below instead.
        }
        let x = origin.x + px(beat as f32 * BEAT_W - scroll_x);
        window.paint_quad(quad(
            gpui::Bounds { origin: point(x, origin.y), size: size(px(1.0), px(height)) },
            px(0.0),
            Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.06 },
            px(0.0),
            gpui::transparent_black(),
            Default::default(),
        ));
    }

    // Bold bar lines — one per bar, numbered by the ruler.
    let first_bar = (scroll_x / bar_w).floor().max(0.0) as i64;
    let last_bar = ((scroll_x + width) / bar_w).ceil() as i64;
    for bar in first_bar..=last_bar {
        let x = origin.x + px(bar as f32 * bar_w - scroll_x);
        window.paint_quad(quad(
            gpui::Bounds { origin: point(x, origin.y), size: size(px(1.0), px(height)) },
            px(0.0),
            Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.2 },
            px(0.0),
            gpui::transparent_black(),
            Default::default(),
        ));
    }

}

/// The Song-mode playhead; meaningless (and hidden) in pattern mode, same
/// as the original's documented behavior. Painted in its own canvas,
/// layered *after* `block_layer` in `render` rather than inside
/// `paint_lanes` (which paints *before* it) — otherwise the playhead line
/// would be stuck drawing underneath every placement block instead of
/// sweeping across on top of them.
fn paint_playhead(window: &mut Window, bounds: gpui::Bounds<gpui::Pixels>, scroll_x: f32, song_step: f64) {
    if song_step < 0.0 {
        return;
    }
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let origin = bounds.origin;
    let song_beat = song_step / 4.0;
    let playhead_x = origin.x + px(song_beat as f32 * BEAT_W - scroll_x);
    if f32::from(playhead_x - origin.x) >= 0.0 && f32::from(playhead_x - origin.x) <= width {
        window.paint_quad(quad(
            gpui::Bounds { origin: point(playhead_x, origin.y), size: size(px(2.0), px(height)) },
            px(0.0),
            Hsla { h: 0.0, s: 0.8, l: 0.6, a: 0.9 },
            px(0.0),
            gpui::transparent_black(),
            Default::default(),
        ));
    }
}

/// A clip block's "what's inside it" content: a min/max-per-pixel-column
/// waveform, read straight off the cached decoded sample — no separate
/// downsampled copy kept around, since this is only ever painted at a
/// handful of pixels wide and recomputing per frame is cheap at that
/// scale. Only the first channel is sampled per bucket (a mono-looking
/// preview is plenty at this size; this isn't the Mixer's own scope).
fn paint_waveform(window: &mut Window, bounds: gpui::Bounds<gpui::Pixels>, sample: &SampleBuffer) {
    let width_px = f32::from(bounds.size.width).max(1.0) as usize;
    let height = f32::from(bounds.size.height);
    let origin = bounds.origin;
    let channels = sample.channels.max(1) as usize;
    let frame_count = sample.frame_count() as usize;
    if frame_count == 0 || width_px == 0 {
        return;
    }
    let frames_per_col = (frame_count / width_px).max(1);
    let mid = height / 2.0;
    let color = theme::accent_dim();
    for col in 0..width_px {
        let start = col * frames_per_col;
        let end = ((col + 1) * frames_per_col).min(frame_count);
        if start >= end {
            continue;
        }
        let mut lo = 0.0f32;
        let mut hi = 0.0f32;
        for frame in start..end {
            let s = sample.data[frame * channels];
            lo = lo.min(s);
            hi = hi.max(s);
        }
        let y0 = mid - hi * mid;
        let y1 = mid - lo * mid;
        let h = (y1 - y0).max(1.0);
        window.paint_quad(quad(
            gpui::Bounds { origin: point(origin.x + px(col as f32), origin.y + px(y0)), size: size(px(1.0), px(h)) },
            px(0.0),
            color,
            px(0.0),
            gpui::transparent_black(),
            Default::default(),
        ));
    }
}

/// A pattern block's "what's inside it" content: every channel's notes
/// for that pattern (`ChannelRack::pattern_notes`, already including any
/// step-sequencer content now that steps are just notes), flattened and
/// scaled into the content area — a simplified, non-interactive cousin of
/// the Piano Roll's own note painting, over a fixed assumed pitch range
/// rather than one computed per block (consistent and cheap, and this
/// preview is far too small for a tight per-block range to matter).
fn paint_mini_notes(window: &mut Window, bounds: gpui::Bounds<gpui::Pixels>, notes: &[Note], length_beats: f64) {
    const PITCH_LO: i32 = 24;
    const PITCH_HI: i32 = 108;
    let length_steps = (length_beats * 4.0).max(1.0);
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let origin = bounds.origin;
    let pitch_span = (PITCH_HI - PITCH_LO).max(1) as f32;
    let color = theme::accent();
    for note in notes {
        let x0 = (note.start / length_steps).clamp(0.0, 1.0) as f32 * width;
        let x1 = ((note.start + note.length) / length_steps).clamp(0.0, 1.0) as f32 * width;
        let w = (x1 - x0).max(1.0);
        let t = 1.0 - (note.pitch.clamp(PITCH_LO, PITCH_HI) - PITCH_LO) as f32 / pitch_span;
        let y = (t * (height - 2.0).max(0.0)).clamp(0.0, (height - 2.0).max(0.0));
        window.paint_quad(quad(
            gpui::Bounds { origin: point(origin.x + px(x0), origin.y + px(y)), size: size(px(w), px(2.0)) },
            px(0.0),
            color,
            px(0.0),
            gpui::transparent_black(),
            Default::default(),
        ));
    }
}
