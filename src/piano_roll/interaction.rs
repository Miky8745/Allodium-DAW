//! Mouse-driven note editing: hit-testing, drag state (move/resize/
//! delete/marquee-select), and the delete/clear-selection actions
//! `AppShell`'s key dispatcher routes in. See `piano_roll.rs`'s own top
//! doc comment for the feature-level history; this file is just the
//! "what a mousedown/mousemove/keypress actually does to `self.notes`"
//! half of it.

use std::collections::HashSet;
use std::time::Instant;

use gpui::{Context, MouseButton, MouseDownEvent, MouseMoveEvent, Point};

use crate::project::Note;

use super::{PianoRoll, PianoRollTool, x_to_step, y_to_pitch};

/// How close to a note's right edge (in content pixels) counts as
/// grabbing it to resize rather than move, Allodium-tauri's own
/// `NoteGrid.svelte` `drawMousedown` threshold (`x >= stepToX(...) - 6`),
/// which lines up directly since this grid's `STEP_W` already equals
/// that file's 1x-zoom `BASE_STEP_W`.
const RESIZE_HANDLE_PX: f32 = 6.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum DragMode {
    None,
    /// Dragging one note or a whole multi-selection by `anchor_note_id`'s
    /// body (freshly placed, or pre-existing and possibly part of a larger
    /// selection). `grab_offset_steps`/`grab_offset_pitch` record where
    /// inside the anchor note the initial click landed, so it doesn't jump
    /// under the cursor; `anchor_initial_*`/`initial_positions` are a
    /// snapshot of every moving note's own position at drag start, so each
    /// frame's new position is computed as "anchor's total displacement
    /// since mousedown, applied to that note's own starting position"
    /// rather than drifting from incremental per-frame deltas. A plain
    /// single-note move is just this with a one-element `initial_positions`.
    Moving {
        anchor_note_id: u32,
        grab_offset_steps: f64,
        grab_offset_pitch: i32,
        anchor_initial_start: f64,
        anchor_initial_pitch: i32,
        initial_positions: Vec<(u32, f64, i32)>,
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
    /// Right-click-drag delete (every tool), or left-click-drag delete
    /// while the Delete tool is active: remove every note the cursor
    /// passes over.
    Deleting,
    /// A box-select drag — Marquee tool, or Ctrl+drag in any tool.
    /// `base_selection` is whatever was already selected at drag start
    /// (kept if the drag began with Shift held, for "add to selection";
    /// otherwise empty), unioned on every move with whichever notes the
    /// box from `(anchor_step, anchor_pitch)` to `(cur_step, cur_pitch)`
    /// currently overlaps.
    Marquee {
        anchor_step: f64,
        anchor_pitch: i32,
        cur_step: f64,
        cur_pitch: i32,
        base_selection: HashSet<u32>,
    },
}

/// Idle-hover classification, purely for cursor affordance, computed on
/// every passive mouse move (not just while dragging) so the resize zone
/// this stage adds is actually discoverable before the user commits to a
/// click.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Hover {
    None,
    Move,
    Resize,
}

pub(super) struct DyingNote {
    pub(super) note: Note,
    pub(super) died_at: Instant,
}

/// Same per-entry-unique-id scheme as `channel_rack/row.rs`'s own
/// `next_note_id` — duplicated rather than shared, since each is private
/// to its own module and the two have no other reason to depend on each
/// other.
fn next_note_id(notes: &[Note]) -> u32 {
    notes.iter().map(|n| n.id).max().unwrap_or(0) + 1
}

impl PianoRoll {
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
    pub(super) fn pointer_to_grid(&self, position: Point<gpui::Pixels>) -> (f64, i32) {
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
    pub(super) fn hit_test(&self, position: Point<gpui::Pixels>) -> (f64, i32, Option<usize>, bool) {
        let (raw_x, _) = self.grid_px(position);
        let (step, pitch) = self.pointer_to_grid(position);
        match self.note_at(step, pitch) {
            Some(idx) => {
                let edge_x = super::step_to_x(self.notes[idx].start + self.notes[idx].length);
                (step, pitch, Some(idx), raw_x >= edge_x - RESIZE_HANDLE_PX)
            }
            None => (step, pitch, None, false),
        }
    }

    /// Starts a box-select drag. `additive` (Shift held at mousedown) keeps
    /// whatever was already selected as the floor every subsequent move
    /// unions new overlaps onto; otherwise it starts from empty — matching
    /// "Shift held *at mousedown* means add to selection", the same
    /// convention `select_row`'s ctrl/shift-click range selection elsewhere
    /// in this app uses for *when* a modifier is read.
    fn begin_marquee(&mut self, step: f64, pitch: i32, additive: bool) {
        let base_selection = if additive { self.selected.clone() } else { HashSet::new() };
        self.drag = DragMode::Marquee { anchor_step: step, anchor_pitch: pitch, cur_step: step, cur_pitch: pitch, base_selection };
        self.update_marquee_selection(step, pitch);
    }

    /// Recomputes `self.selected` as the marquee's `base_selection` unioned
    /// with every note overlapping the box from the drag's anchor to
    /// `(cur_step, cur_pitch)` — called on every mouse move while
    /// `DragMode::Marquee` is active, and once immediately at mousedown
    /// (anchor == cur), so a click-without-dragging still selects whatever
    /// single note sits under the cursor, matching the original's own
    /// "a click without dragging selects the note under the cursor."
    fn update_marquee_selection(&mut self, cur_step: f64, cur_pitch: i32) {
        let (anchor_step, anchor_pitch, base_selection) = match &mut self.drag {
            DragMode::Marquee { anchor_step, anchor_pitch, cur_step: cs, cur_pitch: cp, base_selection } => {
                *cs = cur_step;
                *cp = cur_pitch;
                (*anchor_step, *anchor_pitch, base_selection.clone())
            }
            _ => return,
        };
        let (lo_step, hi_step) = (anchor_step.min(cur_step), anchor_step.max(cur_step));
        let (lo_pitch, hi_pitch) = (anchor_pitch.min(cur_pitch), anchor_pitch.max(cur_pitch));
        let mut selected = base_selection;
        for note in &self.notes {
            let overlaps_step = note.start <= hi_step && note.start + note.length > lo_step;
            let overlaps_pitch = note.pitch >= lo_pitch && note.pitch <= hi_pitch;
            if overlaps_step && overlaps_pitch {
                selected.insert(note.id);
            }
        }
        self.selected = selected;
    }

    pub(super) fn handle_mouse_down(&mut self, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        let Some(channel_id) = self.loaded_channel_id else { return };
        let (step, pitch, hit, on_resize_edge) = self.hit_test(ev.position);

        // Ctrl+drag is always a marquee, on a note or empty space, in
        // every tool — the one gesture that isn't gated on `self.tool`.
        if ev.modifiers.control && ev.button == MouseButton::Left {
            self.begin_marquee(step, pitch, ev.modifiers.shift);
            cx.notify();
            return;
        }

        match ev.button {
            // The right button deletes in every tool, regardless of `self.tool`.
            MouseButton::Right => {
                self.drag = DragMode::Deleting;
                self.delete_at(step, pitch);
                self.commit_live(channel_id, cx);
            }
            MouseButton::Left => match self.tool {
                PianoRollTool::Delete => {
                    self.drag = DragMode::Deleting;
                    self.delete_at(step, pitch);
                    self.commit_live(channel_id, cx);
                }
                PianoRollTool::Marquee => {
                    self.begin_marquee(step, pitch, ev.modifiers.shift);
                    cx.notify();
                }
                PianoRollTool::Draw => {
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
                            let pitch = note.pitch;
                            self.drag = DragMode::Resizing { note_id: note.id, anchor_step: step, orig_length: note.length };
                            self.begin_live_preview(channel_id, pitch, cx);
                        }
                        Some(idx) => {
                            let note_id = self.notes[idx].id;
                            if ev.modifiers.shift {
                                self.selected.insert(note_id);
                            } else if !self.selected.contains(&note_id) || self.selected.len() <= 1 {
                                self.selected = std::iter::once(note_id).collect();
                            }
                            // else: `note_id` is already part of a larger
                            // selection and Shift isn't held — keep it as-is
                            // so the drag below moves the whole group.
                            let note = &self.notes[idx];
                            let (anchor_initial_start, anchor_initial_pitch) = (note.start, note.pitch);
                            let grab_offset_steps = step - note.start;
                            let grab_offset_pitch = pitch - note.pitch;
                            let initial_positions: Vec<(u32, f64, i32)> =
                                self.notes.iter().filter(|n| self.selected.contains(&n.id)).map(|n| (n.id, n.start, n.pitch)).collect();
                            self.drag = DragMode::Moving {
                                anchor_note_id: note_id,
                                grab_offset_steps,
                                grab_offset_pitch,
                                anchor_initial_start,
                                anchor_initial_pitch,
                                initial_positions,
                            };
                            // Clicking (as opposed to placing) an existing
                            // note is the "point at a note, hear and see
                            // it" gesture — light up the key it belongs to
                            // on the real piano sidebar and sound it,
                            // mirroring a real piano key being pressed.
                            self.begin_live_preview(channel_id, anchor_initial_pitch, cx);
                        }
                        None => {
                            // Draw tool, empty space: clears the selection
                            // and places a fresh note at `last_length`,
                            // immediately selected and ready to drag.
                            let max_len = (self.pattern_length_steps as f64 - step).max(1.0);
                            let length = self.last_length.min(max_len);
                            let d = self.defaults;
                            let note = Note {
                                id: next_note_id(&self.notes),
                                pitch,
                                start: step,
                                length,
                                velocity: d.velocity,
                                pan: d.pan,
                                release: d.release,
                                mod_x: d.mod_x,
                                mod_y: d.mod_y,
                                fine_pitch: d.fine_pitch,
                                color: d.color,
                                slide: d.slide,
                            };
                            let note_id = note.id;
                            self.notes.push(note);
                            self.selected = std::iter::once(note_id).collect();
                            self.drag = DragMode::Moving {
                                anchor_note_id: note_id,
                                grab_offset_steps: 0.0,
                                grab_offset_pitch: 0,
                                anchor_initial_start: step,
                                anchor_initial_pitch: pitch,
                                initial_positions: vec![(note_id, step, pitch)],
                            };
                        }
                    }
                    self.commit_live(channel_id, cx);
                }
            },
            _ => {}
        }
    }

    pub(super) fn handle_mouse_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(channel_id) = self.loaded_channel_id else { return };
        let (step, pitch) = self.pointer_to_grid(ev.position);

        match self.drag.clone() {
            DragMode::Moving { grab_offset_steps, grab_offset_pitch, anchor_initial_start, anchor_initial_pitch, initial_positions, .. } => {
                // Every moving note (the whole selection, for a group
                // drag) carries the anchor's total displacement since
                // mousedown, applied to *that note's own* starting
                // position — not accumulated per-frame deltas — so the
                // group's relative shape never drifts.
                let new_anchor_start = step - grab_offset_steps;
                let new_anchor_pitch = pitch - grab_offset_pitch;
                let delta_step = new_anchor_start - anchor_initial_start;
                let delta_pitch = new_anchor_pitch - anchor_initial_pitch;
                for (note_id, orig_start, orig_pitch) in initial_positions {
                    if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
                        let max_start = (self.pattern_length_steps as f64 - note.length).max(0.0);
                        note.start = (orig_start + delta_step).clamp(0.0, max_start);
                        note.pitch = (orig_pitch + delta_pitch).clamp(0, 127);
                    }
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
            DragMode::Marquee { .. } => {
                self.update_marquee_selection(step, pitch);
                cx.notify();
            }
            DragMode::None => {}
        }
    }

    pub(super) fn end_drag(&mut self, cx: &mut Context<Self>) {
        self.end_live_preview(cx);
        if self.drag == DragMode::None {
            return;
        }
        if let DragMode::Moving { anchor_note_id, .. } | DragMode::Resizing { note_id: anchor_note_id, .. } = &self.drag {
            if let Some(note) = self.notes.iter().find(|n| n.id == *anchor_note_id) {
                self.last_length = note.length;
            }
        }
        self.drag = DragMode::None;
        self.channel_rack.update(cx, |rack, _| rack.persist());
    }

    /// Clicking an existing note (move or resize grab) sounds it and lights
    /// its key on the piano sidebar, reusing the QWERTY-piano plumbing
    /// (`ChannelRack::live_note_on`/`held_pitches`) rather than a separate
    /// one-shot preview path, so the key glow and the note-off both come
    /// for free. `live_preview_pitch` remembers which pitch to release,
    /// since a drag can carry the note to a different pitch than it
    /// started at before the mouse button comes back up.
    fn begin_live_preview(&mut self, channel_id: u32, pitch: i32, cx: &mut Context<Self>) {
        self.end_live_preview(cx);
        self.channel_rack.update(cx, |cr, cx| cr.live_note_on(channel_id, pitch, cx));
        self.live_preview_pitch = Some(pitch);
    }

    pub(super) fn end_live_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(pitch) = self.live_preview_pitch.take() {
            self.channel_rack.update(cx, |cr, cx| cr.live_note_off(pitch, cx));
        }
    }

    fn delete_at(&mut self, step: f64, pitch: i32) {
        if let Some(idx) = self.note_at(step, pitch) {
            let note = self.notes.remove(idx);
            self.selected.remove(&note.id);
            self.dying.push(DyingNote { note, died_at: Instant::now() });
        }
    }

    /// Delete/Backspace (routed in from `AppShell`'s own key dispatcher,
    /// `shell.rs`, while this panel is open and frontmost): removes every
    /// selected note, or — with nothing selected — every note of the open
    /// channel in the current pattern, matching the original's own
    /// "Delete with nothing selected clears every note" behavior.
    pub(crate) fn delete_selected_or_clear(&mut self, cx: &mut Context<Self>) {
        let Some(channel_id) = self.loaded_channel_id else { return };
        let now = Instant::now();
        if self.selected.is_empty() {
            for note in self.notes.drain(..) {
                self.dying.push(DyingNote { note, died_at: now });
            }
        } else {
            let selected = std::mem::take(&mut self.selected);
            let mut i = 0;
            while i < self.notes.len() {
                if selected.contains(&self.notes[i].id) {
                    let note = self.notes.remove(i);
                    self.dying.push(DyingNote { note, died_at: now });
                } else {
                    i += 1;
                }
            }
        }
        self.commit_live(channel_id, cx);
        cx.notify();
    }

    /// Escape (also routed in from `AppShell`): clears the selection.
    pub(crate) fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            return;
        }
        self.selected.clear();
        cx.notify();
    }

    pub(super) fn prune_dying(&mut self) {
        self.dying.retain(|d| d.died_at.elapsed().as_secs_f32() < super::DEATH_FADE_SECS);
    }

    pub(super) fn commit_live(&mut self, channel_id: u32, cx: &mut Context<Self>) {
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
}
