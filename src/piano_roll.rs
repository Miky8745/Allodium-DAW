//! Piano Roll: the virtualized note grid, originally ported from
//! `Allodium-GPUI-test/src/grid.rs` + `src/model.rs` (see
//! `../03-reuse-map.md`) and wired to a real channel's real note data
//! instead of a synthetic seeded `Vec<Note>`. See
//! `docs/stage-05-piano-roll.md`.
//!
//! This pass brings its *interaction* behavior up to parity with
//! Allodium-tauri's `NoteGrid.svelte` (`docs/piano-roll.md`) for the
//! pieces that are actually in this rewrite's MVP scope
//! (`daw-rewrite-starter/02-feature-scope.md` item 4: "place/move/
//! resize/delete, basic velocity"), deliberately not a blind port:
//!
//! - **Added**: resizing an existing note by grabbing its last ~6px (the
//!   original's own threshold, and it lines up exactly since this grid's
//!   `STEP_W` already matches the original's `BASE_STEP_W`). This was
//!   simply missing before: there was no way to change a note's length
//!   at all once placed. `last_length` (copied from whatever note was
//!   last moved/resized, used for the next placed note) comes along with
//!   it, both because the original does it and because without it a
//!   freshly placed note could silently overflow a short pattern (the
//!   old code only clamped `start`, not `length`, against
//!   `pattern_length_steps`).
//! - **Added**: a piano-keys sidebar (white/black/C-row banding, C note
//!   labels). Without any pitch reference at all, there was no way to
//!   tell which row a note landed on. Simplified from the original's
//!   `PianoKeys.svelte`: no live drag-highlight (that needs a
//!   highlight-set wired back from the grid's drag state purely for a
//!   cosmetic cue) and no MIDI-input glow (no MIDI I/O in this rewrite).
//! - **Added**: hover cursor affordance (grab/resize/not-allowed), so the
//!   new resize zone is actually discoverable, not just functional.
//! - **Declined, matching `02-feature-scope.md`'s own stretch list**:
//!   zoom, marquee/multi-select, copy/paste, note properties beyond
//!   velocity, slide notes, auto-extend pattern length, the velocity
//!   lane, the overview/minimap strip, and per-channel independent-loop
//!   playhead wrapping. Also declined: pitched audio preview on
//!   place/move (the original's `audioEngine.previewNote`). This
//!   engine's `AudioEngine::preview` has no pitch parameter yet, and
//!   adding one is a real engine change, not a Piano Roll one.
//! - **Declined, not a real bug to carry forward**: the original docs
//!   themselves flag a historical bug: `NoteData.start`/`length` used
//!   to be `u32`, which broke saving fractional notes before they were
//!   changed to `f64`. This rewrite's `Note` (`src/project.rs`) was
//!   `f64` from Stage 2 onward, so there's nothing to port here.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use gpui::component::button::{Button, ButtonVariants};
use gpui::component::menu::{DropdownMenu, PopupMenuItem};
use gpui::{
    Context, Entity, Hsla, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, Render, ScrollEvent, Styled, Window, canvas, div, linear_color_stop,
    linear_gradient, point, prelude::*, px, quad, size,
};

use crate::channel_rack::ChannelRack;
use crate::project::Note;
use crate::theme;

const PITCH_COUNT: i32 = 128;
const KEY_H: f32 = 18.0;
const STEP_W: f32 = 24.0;
const KEY_COL_W: f32 = 56.0;
const DEFAULT_NOTE_LEN: f64 = 8.0;
const DEFAULT_VELOCITY: f64 = 1.0;
const DEATH_FADE_SECS: f32 = 0.3;
/// How close to a note's right edge (in content pixels) counts as
/// grabbing it to resize rather than move, Allodium-tauri's own
/// `NoteGrid.svelte` `drawMousedown` threshold (`x >= stepToX(...) - 6`),
/// which lines up directly since this grid's `STEP_W` already equals
/// that file's 1x-zoom `BASE_STEP_W`.
const RESIZE_HANDLE_PX: f32 = 6.0;

fn pitch_to_y(pitch: i32) -> f32 {
    (127 - pitch) as f32 * KEY_H
}

fn y_to_pitch(y: f32) -> i32 {
    let row = (y / KEY_H).floor();
    (127 - row as i32).clamp(0, 127)
}

fn step_to_x(step: f64) -> f32 {
    step as f32 * STEP_W
}

fn x_to_step(x: f32) -> f64 {
    (x / STEP_W).floor().max(0.0) as f64
}

fn is_black_key(pitch: i32) -> bool {
    matches!(pitch.rem_euclid(12), 1 | 3 | 6 | 8 | 10)
}

fn next_note_id(notes: &[Note]) -> u32 {
    notes.iter().map(|n| n.id).max().unwrap_or(0) + 1
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DragMode {
    None,
    /// Dragging a note (freshly placed or pre-existing) by its body.
    /// `grab_offset_steps`/`grab_offset_pitch` record where inside the note
    /// the initial click landed, so the note doesn't jump under the cursor.
    Moving {
        note_id: u32,
        grab_offset_steps: f64,
        grab_offset_pitch: i32,
    },
    /// Dragging a note's right edge to change its length. Pitch stays
    /// locked, only `length` changes, anchored to the step the drag
    /// started on so the edge tracks the cursor exactly rather than
    /// jumping to wherever the cursor happens to be.
    Resizing {
        note_id: u32,
        anchor_step: f64,
        orig_length: f64,
    },
    /// Right-click-drag delete: remove every note the cursor passes over.
    Deleting,
}

/// Idle-hover classification, purely for cursor affordance, computed on
/// every passive mouse move (not just while dragging) so the resize zone
/// this stage adds is actually discoverable before the user commits to a
/// click.
#[derive(Clone, Copy, PartialEq)]
enum Hover {
    None,
    Move,
    Resize,
}

struct DyingNote {
    note: Note,
    died_at: Instant,
}

pub struct PianoRoll {
    channel_rack: Entity<ChannelRack>,
    loaded_channel_id: Option<u32>,
    notes: Vec<Note>,
    pattern_length_steps: u32,
    scroll_x: f32,
    scroll_y: f32,
    drag: DragMode,
    hover: Hover,
    /// The length a freshly placed note gets, clamped against
    /// `pattern_length_steps` so it can never overflow a short pattern.
    /// Updated to whatever note was last moved/resized, mirroring
    /// `NoteGrid.svelte`'s own `lastLength`. Worth carrying over, not
    /// just for cosmetic parity: without the clamp half of this, a
    /// short pattern could get a note that overflows it.
    last_length: f64,
    dying: Vec<DyingNote>,
    /// The grid canvas's own on-screen origin, captured from its `canvas`
    /// paint closure each frame. Plain `div()`s don't hand their own
    /// screen bounds to mouse-event handlers (only `canvas`'s prepaint/
    /// paint closures get a `bounds` param, see `docs/gpui-api-notes.md`),
    /// and unlike the prototype this grid no longer fills the whole
    /// window (it sits below the toolbar/Channel Rack/its own header), so
    /// `event.position` (window-content coordinates) needs this subtracted
    /// before it maps onto grid-local pixels.
    grid_origin: Rc<Cell<Point<Pixels>>>,
    /// The grid canvas's own on-screen size, captured alongside
    /// `grid_origin` from the same paint closure. `clamp_scroll` needs the
    /// *visible grid area's* height, not `window.viewport_size()` (the
    /// whole OS window, which is always taller than this floating panel's
    /// own grid area): using the window size there under-clamped `scroll_y`
    /// and made the view stop short of the lowest row (C0) ever being
    /// reachable.
    grid_size: Rc<Cell<gpui::Size<Pixels>>>,
}

impl PianoRoll {
    pub fn new(channel_rack: Entity<ChannelRack>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            channel_rack,
            loaded_channel_id: None,
            notes: Vec::new(),
            pattern_length_steps: 16,
            scroll_x: 0.0,
            scroll_y: (PITCH_COUNT as f32 * KEY_H) / 2.0 - 200.0,
            drag: DragMode::None,
            hover: Hover::None,
            last_length: DEFAULT_NOTE_LEN,
            dying: Vec::new(),
            grid_origin: Rc::new(Cell::new(point(px(0.0), px(0.0)))),
            grid_size: Rc::new(Cell::new(size(px(0.0), px(0.0)))),
        }
    }

    /// Reloads `notes` from the shared project model whenever the selected
    /// channel changes (including the first render). Edits made through
    /// this view are the source of truth for the currently loaded channel
    /// in between reloads, so this never clobbers an in-progress drag.
    fn sync_selection(&mut self, cx: &mut Context<Self>) {
        let rack = self.channel_rack.read(cx);
        let selected = rack.selected_channel_id();
        self.pattern_length_steps = rack.pattern_length_steps();
        if selected != self.loaded_channel_id {
            self.loaded_channel_id = selected;
            self.notes = selected.map(|id| rack.notes_for(id)).unwrap_or_default();
        }
    }

    fn content_width(&self) -> f32 {
        self.pattern_length_steps as f32 * STEP_W
    }

    fn content_height(&self) -> f32 {
        PITCH_COUNT as f32 * KEY_H
    }

    /// Clamps scroll so the viewport never scrolls past the end of the
    /// content (max scroll = content size minus the visible viewport size).
    fn clamp_scroll(&mut self, viewport: gpui::Size<gpui::Pixels>) {
        let max_x = (self.content_width() - f32::from(viewport.width)).max(0.0);
        let max_y = (self.content_height() - f32::from(viewport.height)).max(0.0);
        self.scroll_x = self.scroll_x.clamp(0.0, max_x);
        self.scroll_y = self.scroll_y.clamp(0.0, max_y);
    }

    fn note_at(&self, step: f64, pitch: i32) -> Option<usize> {
        self.notes
            .iter()
            .position(|n| n.pitch == pitch && step >= n.start && step < n.start + n.length)
    }

    /// `position` is in window-content coordinates; the grid canvas is
    /// offset from the window origin (toolbar rows, Channel Rack panel,
    /// this view's own header), so its on-screen origin (captured from the
    /// canvas's paint closure into `grid_origin`) has to be subtracted
    /// before the scroll offset maps it onto grid-local pixels.
    fn pointer_to_grid(&self, position: Point<gpui::Pixels>) -> (f64, i32) {
        let (x, _) = self.grid_px(position);
        let pitch = y_to_pitch(f32::from(position.y - self.grid_origin.get().y) + self.scroll_y);
        (x_to_step(x), pitch)
    }

    /// Raw (unsnapped) grid-local pixel position, in content space (scroll
    /// already added back in). Needed for the resize-edge hit test, which
    /// cares about the exact pixel the cursor is at relative to a note's
    /// right edge, not which whole step that pixel floors to.
    fn grid_px(&self, position: Point<gpui::Pixels>) -> (f32, f32) {
        let origin = self.grid_origin.get();
        (f32::from(position.x - origin.x) + self.scroll_x, f32::from(position.y - origin.y) + self.scroll_y)
    }

    /// Classifies what a click/hover at `position` would act on: the
    /// resize zone of an existing note, its body, or empty space. Shared
    /// between the real mousedown handler and the idle-hover cursor logic
    /// so the two can never disagree about where the resize zone is.
    fn hit_test(&self, position: Point<gpui::Pixels>) -> (f64, i32, Option<usize>, bool) {
        let (raw_x, _) = self.grid_px(position);
        let (step, pitch) = self.pointer_to_grid(position);
        match self.note_at(step, pitch) {
            Some(idx) => {
                let edge_x = step_to_x(self.notes[idx].start + self.notes[idx].length);
                (step, pitch, Some(idx), raw_x >= edge_x - RESIZE_HANDLE_PX)
            }
            None => (step, pitch, None, false),
        }
    }

    fn handle_mouse_down(&mut self, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        let Some(channel_id) = self.loaded_channel_id else { return };
        let (step, pitch, hit, on_resize_edge) = self.hit_test(ev.position);

        match ev.button {
            MouseButton::Right => {
                self.drag = DragMode::Deleting;
                self.delete_at(step, pitch);
            }
            MouseButton::Left => {
                // Clicking past the pattern's own end is a no-op,
                // mirroring `NoteGrid.svelte`'s `drawMousedown` early
                // return (`stepSnapped >= patternLength`); without it, a
                // click past the last step could place or drag a note no
                // scheduler position will ever reach.
                if step >= self.pattern_length_steps as f64 {
                    return;
                }
                match hit {
                    Some(idx) if on_resize_edge => {
                        let note = &self.notes[idx];
                        self.drag = DragMode::Resizing { note_id: note.id, anchor_step: step, orig_length: note.length };
                    }
                    Some(idx) => {
                        let note = &self.notes[idx];
                        self.drag = DragMode::Moving {
                            note_id: note.id,
                            grab_offset_steps: step - note.start,
                            grab_offset_pitch: pitch - note.pitch,
                        };
                    }
                    None => {
                        let max_len = (self.pattern_length_steps as f64 - step).max(1.0);
                        let length = self.last_length.min(max_len);
                        let note = Note { id: next_note_id(&self.notes), pitch, start: step, length, velocity: DEFAULT_VELOCITY };
                        let note_id = note.id;
                        self.notes.push(note);
                        self.drag = DragMode::Moving { note_id, grab_offset_steps: 0.0, grab_offset_pitch: 0 };
                    }
                }
            }
            _ => return,
        }
        self.commit_live(channel_id, cx);
    }

    fn handle_mouse_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(channel_id) = self.loaded_channel_id else { return };
        let (step, pitch) = self.pointer_to_grid(ev.position);

        match self.drag {
            DragMode::Moving { note_id, grab_offset_steps, grab_offset_pitch } => {
                if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
                    let max_start = (self.pattern_length_steps as f64 - note.length).max(0.0);
                    note.start = (step - grab_offset_steps).clamp(0.0, max_start);
                    note.pitch = (pitch - grab_offset_pitch).clamp(0, 127);
                }
                self.commit_live(channel_id, cx);
            }
            DragMode::Resizing { note_id, anchor_step, orig_length } => {
                if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
                    let max_len = (self.pattern_length_steps as f64 - note.start).max(1.0);
                    note.length = (orig_length + (step - anchor_step)).clamp(1.0, max_len);
                }
                self.commit_live(channel_id, cx);
            }
            DragMode::Deleting => {
                self.delete_at(step, pitch);
                self.commit_live(channel_id, cx);
            }
            DragMode::None => {}
        }
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag == DragMode::None {
            return;
        }
        if let DragMode::Moving { note_id, .. } | DragMode::Resizing { note_id, .. } = self.drag {
            if let Some(note) = self.notes.iter().find(|n| n.id == note_id) {
                self.last_length = note.length;
            }
        }
        self.drag = DragMode::None;
        self.channel_rack.update(cx, |rack, _| rack.persist());
    }

    fn delete_at(&mut self, step: f64, pitch: i32) {
        if let Some(idx) = self.note_at(step, pitch) {
            let note = self.notes.remove(idx);
            self.dying.push(DyingNote { note, died_at: Instant::now() });
        }
    }

    fn prune_dying(&mut self) {
        self.dying.retain(|d| d.died_at.elapsed().as_secs_f32() < DEATH_FADE_SECS);
    }

    fn commit_live(&mut self, channel_id: u32, cx: &mut Context<Self>) {
        let notes = self.notes.clone();
        self.channel_rack.update(cx, |rack, cx| {
            rack.update_notes(channel_id, notes, cx);
            // Grows the pattern once a note reaches its current last bar,
            // or shrinks it back down once trailing bars go empty again
            // (e.g. after a delete) — always exactly one spare empty bar
            // past the content, across every channel, not just this one.
            rack.sync_pattern_length(cx);
        });
        self.pattern_length_steps = self.channel_rack.read(cx).pattern_length_steps();
    }

    /// A channel-picker dropdown — an easier way to switch which channel
    /// the Piano Roll shows than hunting down its row in the Channel Rack
    /// and double-clicking it. It's not a second notion of "which channel
    /// is open": picking an item here just calls the same
    /// `ChannelRack::select_channel` that double-click already does, so
    /// `sync_selection`'s existing live-follow keeps working unchanged —
    /// there is still only one "selected channel" in the whole app.
    fn render_channel_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let rack = self.channel_rack.clone();
        let current = self.loaded_channel_id;
        let channels: Vec<(u32, String)> = self
            .channel_rack
            .read(cx)
            .channels()
            .iter()
            .filter(|c| !c.sample_path.is_empty())
            .map(|c| (c.id, c.name.clone()))
            .collect();
        let current_label =
            channels.iter().find(|(id, _)| Some(*id) == current).map(|(_, l)| l.clone()).unwrap_or_else(|| "No channel".to_string());

        div()
            .h(px(28.0))
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .bg(theme::HEADER_BG)
            .border_b_1()
            .border_color(theme::BORDER)
            .child(div().text_size(px(10.0)).text_color(theme::accent_dim()).child("Channel"))
            .child(
                Button::new("piano-roll-channel").ghost().compact().label(current_label).dropdown_menu(move |menu, _, _| {
                    channels.iter().cloned().fold(menu, |menu, (id, label)| {
                        let rack = rack.clone();
                        menu.item(PopupMenuItem::new(label).checked(Some(id) == current).on_click(move |_, _, cx| {
                            rack.update(cx, |r, cx| r.select_channel(id, cx));
                        }))
                    })
                }),
            )
    }
}

impl Render for PianoRoll {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_selection(cx);
        self.clamp_scroll(self.grid_size.get());
        self.prune_dying();
        window.request_animation_frame();

        let rack = self.channel_rack.read(cx);
        let playing = rack.is_playing();
        let current_step = if playing { rack.current_step() } else { -1.0 };

        // Every other channel's notes, shown dimmed and read-only behind
        // the focused channel's own — "if there is something in a channel
        // you are not focusing, display this channel's contents as light
        // gray unmovable notes." Never pushed into `self.notes`, so
        // `hit_test`/`note_at` (which only ever scan that field) can't
        // select or drag them.
        let background_notes: Vec<Note> = rack
            .channels()
            .iter()
            .filter(|c| Some(c.id) != self.loaded_channel_id)
            .flat_map(|c| rack.notes_for(c.id))
            .collect();

        let notes = self.notes.clone();
        let dying: Vec<(Note, f32)> = self
            .dying
            .iter()
            .map(|d| (d.note.clone(), d.died_at.elapsed().as_secs_f32() / DEATH_FADE_SECS))
            .collect();
        let scroll_x = self.scroll_x;
        let scroll_y = self.scroll_y;
        let grid_origin = self.grid_origin.clone();
        let grid_size = self.grid_size.clone();

        // Cursor affordance: an active drag wins outright; otherwise it
        // reflects the idle hover classification `handle_mouse_move`'s
        // passive branch below keeps up to date. This is what makes the
        // resize zone this stage adds actually discoverable rather than
        // a hitbox nobody would find by accident.
        let grid_area = div()
            .size_full()
            .child(
                canvas(
                    move |_, _, _| {},
                    move |bounds, _, window, _| {
                        grid_origin.set(bounds.origin);
                        grid_size.set(bounds.size);
                        paint_grid(window, bounds, scroll_x, scroll_y, &notes, &background_notes, &dying, current_step);
                    },
                )
                .size_full(),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _, cx| this.handle_mouse_down(ev, cx)))
            .on_mouse_down(MouseButton::Right, cx.listener(|this, ev: &MouseDownEvent, _, cx| this.handle_mouse_down(ev, cx)))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if this.drag != DragMode::None {
                    this.handle_mouse_move(ev, cx);
                    return;
                }
                let hover = if this.loaded_channel_id.is_none() {
                    Hover::None
                } else {
                    let (_, _, hit, on_resize_edge) = this.hit_test(ev.position);
                    match hit {
                        Some(_) if on_resize_edge => Hover::Resize,
                        Some(_) => Hover::Move,
                        None => Hover::None,
                    }
                };
                if hover != this.hover {
                    this.hover = hover;
                    cx.notify();
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
            );
        let grid_area = match self.drag {
            DragMode::Moving { .. } => grid_area.cursor_grabbing(),
            DragMode::Resizing { .. } => grid_area.cursor_col_resize(),
            DragMode::Deleting => grid_area.cursor_not_allowed(),
            DragMode::None => match self.hover {
                Hover::Move => grid_area.cursor_grab(),
                Hover::Resize => grid_area.cursor_col_resize(),
                Hover::None => grid_area.cursor_default(),
            },
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::BG_APP)
            .child(self.render_channel_picker(cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .w_full()
                    .flex()
                    .overflow_hidden()
                    .child(piano_keys(self.scroll_y, self.channel_rack.read(cx).held_pitches()))
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .bg(theme::PIANO_GRID_BG)
                            .on_scroll(cx.listener(|this, ev: &ScrollEvent, _window, cx| {
                                let delta = ev.delta.pixel_delta(px(KEY_H));
                                this.scroll_x -= f32::from(delta.x);
                                this.scroll_y -= f32::from(delta.y);
                                this.clamp_scroll(this.grid_size.get());
                                cx.notify();
                            }))
                            .child(grid_area),
                    ),
            )
    }
}

/// A fixed-width sidebar to the grid's left: white/black/C-row banding
/// and C-note labels, scrolled in lockstep with the grid's own
/// `scroll_y` via a plain `top` offset, the same "no bound scroll
/// container, just translate the content" trick `NoteGrid.svelte`'s own
/// `PianoKeys`/`PianoRollRuler` use to stay in sync without a second
/// independently-scrollable element. Plain `div`s (not the canvas the
/// grid needs for its mouse-position math), since this column has no
/// interaction of its own and `div().child(label)` gets real text for
/// free, unlike painting it inside a `canvas`.
///
/// Every row is still a uniform `KEY_H`-tall band (so it lines up 1:1
/// with the note grid), but C#'s row fakes the look of a real keyboard,
/// where the grey C key's body visually extends up into the lower half
/// of C#, by painting a hard-stop gradient (top half plain white, bottom
/// half the same grey as the C row below it) instead of a flat fill.
/// Ported from the same illusion in `PianoKeys.svelte`
/// (`.key-row--csharp`'s `linear-gradient(..., 50%, 50%)`), not re-derived.
fn piano_keys(scroll_y: f32, held: &std::collections::HashSet<i32>) -> impl IntoElement {
    let rows = (0..PITCH_COUNT).map(|row| {
        let pitch = 127 - row;
        let is_black = is_black_key(pitch);
        let is_c = pitch.rem_euclid(12) == 0;
        let is_csharp = pitch.rem_euclid(12) == 1;
        // Suppress the seam between a white key and the black key
        // sitting on/beside it, matching `PianoKeys.svelte`'s
        // `noBorder = isBlackKey(p) || isBlackKey(belowP)`: only true
        // white-to-white boundaries (E/F, B/C) keep a border line.
        let no_border = is_black || is_black_key(pitch - 1);
        // Live-note glow (QWERTY "typing keyboard to piano", see
        // `toolbar::AppShell::on_qwerty_key_down`) — `PianoKeys.svelte`'s
        // `highlighted`/`csharpBelowCHighlighted`, minus the velocity-
        // scaled MIDI glow (no MIDI I/O here, just on/off).
        let highlighted = !is_black && held.contains(&pitch);
        let csharp_below_c_highlighted = is_csharp && held.contains(&(pitch - 1));

        let key = div().relative().w_full().h(px(KEY_H)).flex_shrink_0();
        let key = if is_csharp {
            let bottom = if csharp_below_c_highlighted { theme::warm_accent() } else { theme::PIANO_KEY_C };
            key.bg(linear_gradient(180.0, linear_color_stop(theme::PIANO_KEY_WHITE, 0.5), linear_color_stop(bottom, 0.5)))
        } else {
            let row_bg = if highlighted { theme::warm_accent() } else if is_c { theme::PIANO_KEY_C } else { theme::PIANO_KEY_WHITE };
            key.bg(row_bg)
        };

        key.when(!no_border, |d| d.border_b_1().border_color(theme::PIANO_GRID_BG.alpha(0.4)))
            .when(is_black, |d| {
                let black_bg = if held.contains(&pitch) { theme::warm_accent() } else { theme::PIANO_KEY_BLACK };
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(2.0))
                        .bottom(px(2.0))
                        .w(px(KEY_COL_W * 0.62))
                        .bg(black_bg)
                        .rounded_r(theme::RADIUS_XS),
                )
            })
            .when(is_c, |d| {
                d.flex().items_center().justify_end().pr_1().child(
                    div().text_size(px(9.0)).text_color(theme::PIANO_GRID_BG.alpha(0.8)).child(format!("C{}", pitch / 12)),
                )
            })
    });

    div()
        .id("piano-keys")
        .w(px(KEY_COL_W))
        .h_full()
        .flex_shrink_0()
        .overflow_hidden()
        .bg(theme::PIANO_KEY_WHITE)
        .child(
        div().relative().top(px(-scroll_y)).flex().flex_col().children(rows),
    )
}

/// All custom painting for one frame: row striping, beat-group banding,
/// notes, dying-note flash, and the playhead overlay. Only the
/// rows/columns/notes intersecting `bounds` (the viewport) are ever turned
/// into draw calls - this is the hand-built virtualization, ported from
/// `Allodium-GPUI-test/src/grid.rs`.
fn paint_grid(
    window: &mut Window,
    bounds: gpui::Bounds<gpui::Pixels>,
    scroll_x: f32,
    scroll_y: f32,
    notes: &[Note],
    background_notes: &[Note],
    dying: &[(Note, f32)],
    current_step: f64,
) {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let origin = bounds.origin;

    let first_row = (scroll_y / KEY_H).floor().max(0.0) as i32;
    let last_row = ((scroll_y + height) / KEY_H).ceil() as i32;
    let first_step = (scroll_x / STEP_W).floor().max(0.0) as i32;
    let last_step = ((scroll_x + width) / STEP_W).ceil() as i32;

    // Beat-group column banding (step axis), drawn first as a base layer.
    for step_group_start in (first_step - (first_step % 4).max(0)..=last_step).step_by(4) {
        if step_group_start < 0 {
            continue;
        }
        let group = step_group_start / 4;
        if group % 2 != 0 {
            continue;
        }
        let x = origin.x + px(step_to_x(step_group_start as f64) - scroll_x);
        let band = quad(
            gpui::Bounds { origin: point(x, origin.y), size: size(px(STEP_W * 4.0), px(height)) },
            px(0.0),
            theme::PIANO_BEAT_BAND,
            px(0.0),
            gpui::transparent_black(),
            Default::default(),
        );
        window.paint_quad(band);
    }

    // Row striping (pitch axis): black-key rows get a darker tint. `row` is
    // a top-down content row index (row 0 = pitch 127); convert to a pitch
    // before using it as one.
    for row in first_row.max(0)..=last_row.min(127) {
        let pitch = 127 - row;
        if !is_black_key(pitch) {
            continue;
        }
        let y = origin.y + px(row as f32 * KEY_H - scroll_y);
        let stripe = quad(
            gpui::Bounds { origin: point(origin.x, y), size: size(px(width), px(KEY_H)) },
            px(0.0),
            theme::PIANO_BLACK_ROW_TINT,
            px(0.0),
            gpui::transparent_black(),
            Default::default(),
        );
        window.paint_quad(stripe);
    }

    let visible_start = first_step.max(0) as f64;
    let visible_end = last_step.max(0) as f64;
    let first_visible_pitch = 127 - last_row.min(127).max(0);
    let last_visible_pitch = 127 - first_row.min(127).max(0);

    // Other channels' notes, dimmed and drawn first so the focused
    // channel's own (real, draggable) notes always land on top of them.
    let ghost_color = Hsla { h: 0.0, s: 0.0, l: 0.75, a: 0.35 };
    for note in background_notes {
        if note.start + note.length <= visible_start || note.start > visible_end {
            continue;
        }
        if note.pitch < first_visible_pitch - 1 || note.pitch > last_visible_pitch + 1 {
            continue;
        }
        paint_note(window, origin, scroll_x, scroll_y, note, ghost_color, 1.0);
    }

    let note_color = theme::accent();
    for note in notes {
        if note.start + note.length <= visible_start || note.start > visible_end {
            continue;
        }
        if note.pitch < first_visible_pitch - 1 || note.pitch > last_visible_pitch + 1 {
            continue;
        }
        paint_note(window, origin, scroll_x, scroll_y, note, note_color, 1.0);
    }

    // Dying notes: a brief flash that fades and shrinks slightly.
    for (note, progress) in dying {
        let alpha = (1.0 - progress).max(0.0);
        let mut color = theme::warm_accent();
        color.a = alpha;
        paint_note(window, origin, scroll_x, scroll_y, note, color, alpha);
    }

    // Playhead overlay, drawn last so it's always on top; tracks scroll.
    if current_step >= 0.0 {
        let playhead_x = origin.x
            + px(step_to_x(current_step.floor()) - scroll_x + (current_step - current_step.floor()) as f32 * STEP_W);
        if f32::from(playhead_x - origin.x) >= 0.0 && f32::from(playhead_x - origin.x) <= width {
            window.paint_quad(quad(
                gpui::Bounds { origin: point(playhead_x, origin.y), size: size(px(2.0), px(height)) },
                px(0.0),
                theme::PIANO_PLAYHEAD,
                px(0.0),
                gpui::transparent_black(),
                Default::default(),
            ));
        }
    }
}

fn paint_note(
    window: &mut Window,
    origin: Point<gpui::Pixels>,
    scroll_x: f32,
    scroll_y: f32,
    note: &Note,
    color: Hsla,
    scale: f32,
) {
    let x = origin.x + px(step_to_x(note.start) - scroll_x + 0.5);
    let y = origin.y + px(pitch_to_y(note.pitch) - scroll_y + 0.5);
    let w = (note.length as f32 * STEP_W - 1.0).max(1.0) * scale;
    let h = (KEY_H - 1.0).max(1.0) * scale;
    window.paint_quad(quad(
        gpui::Bounds { origin: point(x, y), size: size(px(w), px(h)) },
        px(2.0),
        color,
        px(0.0),
        gpui::transparent_black(),
        Default::default(),
    ));
}
