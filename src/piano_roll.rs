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
//! - **Added** (later pass, at the user's explicit request to go beyond
//!   `02-feature-scope.md`'s own stretch-list deferral): the toolbar from
//!   Allodium-tauri's `PianoRollHeader.svelte` — Draw/Delete/Marquee tool
//!   buttons (`PianoRollTool`), a Play button that solos and plays just
//!   the open channel (`toggle_solo_play`), and a Note Properties toggle.
//!   Marquee is a real box-select (`DragMode::Marquee`,
//!   `update_marquee_selection`) with Ctrl+drag available in every tool,
//!   group-move for a multi-note selection, and Delete/Backspace/Escape
//!   routed in from `AppShell`'s key dispatcher (`shell.rs`'s
//!   `route_piano_roll_key`) while this panel is open and frontmost.
//! - **Added** (same later pass, extended further at the user's explicit
//!   request): the rest of the original's `NotePropertiesPanel` —
//!   `Note` gained `pan`/`release`/`mod_x`/`mod_y`/`fine_pitch`/`color`/
//!   `slide` (`src/project.rs`, all `#[serde(default)]` so older saves
//!   keep loading), the properties panel (`render_note_properties`)
//!   edits all of them via `NoteParam` + a color swatch picker + a slide
//!   toggle, and notes draw in their own color in the grid
//!   (`paint_grid`'s own note loop) with a bright rim standing in for the
//!   original's white-rim/glow/diagonal-stripe selected-note treatment
//!   (no stripe-drawing primitive here). The audio engine (`src/audio/
//!   mod.rs`) actually applies all of these now, not just storing them:
//!   `trigger_channel_step` offsets the channel's own pan by `note.pan`,
//!   shifts playback rate by `note.fine_pitch` cents
//!   (`cents_to_rate_multiplier`), runs a per-voice RBJ lowpass biquad
//!   from `mod_x`/`mod_y` (`BiquadLowpass`, bypassed entirely at the
//!   default "fully open" cutoff so the common case costs nothing extra
//!   per sample), extends the envelope's release tail past the note's
//!   own end for `release` values above the `0.5` default
//!   (`Envelope::shaped_with_release`), and — a real engine change, not
//!   just the Piano Roll — a `slide` note never starts a voice of its
//!   own; instead it bends every still-ringing voice of the same channel
//!   and note color to its own pitch over its own length
//!   (`Voice::begin_slide`/`ChannelVoiceData::id`). Mixed-value display
//!   (the original's dimmed "MIXED" label when selected notes disagree)
//!   is skipped — the panel just shows the first selected note's value
//!   per control.
//! - **Declined, matching `02-feature-scope.md`'s own stretch list**:
//!   zoom, copy/paste, the velocity lane, the overview/minimap strip,
//!   and per-channel independent-loop playhead wrapping. Also declined:
//!   pitched audio preview on place/move (the original's `audioEngine.
//!   previewNote`) — unaffected by the note-properties audio work above,
//!   since preview voices (`AudioEngine::preview`) never go through
//!   `trigger_channel_step` at all. Also declined (simplification, not a
//!   missed spec item): the original's "click without dragging on a
//!   note inside a multi-selection narrows the selection to just it," and
//!   its slide-voice-registration race (two voices racing to apply a
//!   bend because samples load asynchronously) — this rewrite's samples
//!   are already decoded and in memory before any note can be
//!   scheduled, so that race can't happen here to begin with.
//! - **Declined, not a real bug to carry forward**: the original docs
//!   themselves flag a historical bug: `NoteData.start`/`length` used
//!   to be `u32`, which broke saving fractional notes before they were
//!   changed to `f64`. This rewrite's `Note` (`src/project.rs`) was
//!   `f64` from Stage 2 onward, so there's nothing to port here.

use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Instant;

use gpui::assets::IconName;
use gpui::component::button::{Button, ButtonVariants};
use gpui::component::menu::{DropdownMenu, PopupMenuItem};
use gpui::component::{Icon, Sizable};
use gpui::{
    Context, Entity, Hsla, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, Render, ScrollEvent, Styled, Window, canvas, div, linear_color_stop,
    linear_gradient, point, prelude::*, px, quad, size,
};

use crate::channel_rack::ChannelRack;
use crate::project::Note;
use crate::theme;
use crate::widgets::icon_toggle;

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

#[derive(Clone, Debug, PartialEq)]
enum DragMode {
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
enum Hover {
    None,
    Move,
    Resize,
}

/// The toolbar's tool selector — Allodium-tauri's own `EditTool` type
/// (`editTools.ts`), shared there with the Playlist; here it's just the
/// Piano Roll's own copy since the Playlist isn't in this rewrite's tool
/// system yet. Same semantics: Draw places/selects/moves/resizes notes,
/// Delete click-or-drags notes away, Marquee always drags a selection box.
/// The right mouse button deletes in every tool regardless of which of
/// these is active, and Ctrl+drag is always a marquee regardless of tool —
/// both handled directly in `handle_mouse_down`, not through this enum.
#[derive(Clone, Copy, PartialEq, Debug)]
enum PianoRollTool {
    Draw,
    Delete,
    Marquee,
}

struct DyingNote {
    note: Note,
    died_at: Instant,
}

/// Every per-note dial in `render_note_properties` edits one of these —
/// shared getter/setter/display logic instead of six near-identical
/// dials, same reasoning `channel_rack::drag::ParamKind` already uses for
/// Pan/Volume. Velocity aside (already an MVP field), these are all the
/// "note properties beyond velocity" this module's own top doc comment
/// used to list as declined.
#[derive(Clone, Copy, PartialEq)]
enum NoteParam {
    Velocity,
    /// Offset on top of the channel's own pan — `Note::pan`.
    Pan,
    /// `Note::release` — how far the release tail rings past the note's
    /// own scheduled end.
    Release,
    /// Filter cutoff — `Note::mod_x`.
    Cutoff,
    /// Filter resonance/Q — `Note::mod_y`.
    Resonance,
    /// Fine pitch in cents ("pitch bend") — `Note::fine_pitch`.
    FinePitch,
}

impl NoteParam {
    const ALL: [NoteParam; 6] =
        [NoteParam::Velocity, NoteParam::Pan, NoteParam::Release, NoteParam::Cutoff, NoteParam::Resonance, NoteParam::FinePitch];

    fn name(self) -> &'static str {
        match self {
            NoteParam::Velocity => "Velocity",
            NoteParam::Pan => "Pan",
            NoteParam::Release => "Release",
            NoteParam::Cutoff => "Cutoff",
            NoteParam::Resonance => "Resonance",
            NoteParam::FinePitch => "Pitch bend",
        }
    }

    fn min(self) -> f64 {
        if self == NoteParam::FinePitch { -1200.0 } else { 0.0 }
    }

    fn max(self) -> f64 {
        if self == NoteParam::FinePitch { 1200.0 } else { 1.0 }
    }

    /// The value a dial resets to on double-click/"Reset" — distinct
    /// from `min()` since several of these (pan, release, cutoff) have a
    /// "neutral/no effect" value in the middle or at the top of their
    /// range, not at zero.
    fn reset_value(self) -> f64 {
        match self {
            NoteParam::Velocity => 1.0,
            NoteParam::Pan => 0.5,
            NoteParam::Release => 0.5,
            NoteParam::Cutoff => 1.0,
            NoteParam::Resonance => 0.0,
            NoteParam::FinePitch => 0.0,
        }
    }

    fn get(self, note: &Note) -> f64 {
        match self {
            NoteParam::Velocity => note.velocity,
            NoteParam::Pan => note.pan,
            NoteParam::Release => note.release,
            NoteParam::Cutoff => note.mod_x,
            NoteParam::Resonance => note.mod_y,
            NoteParam::FinePitch => note.fine_pitch,
        }
    }

    fn set(self, note: &mut Note, value: f64) {
        match self {
            NoteParam::Velocity => note.velocity = value,
            NoteParam::Pan => note.pan = value,
            NoteParam::Release => note.release = value,
            NoteParam::Cutoff => note.mod_x = value,
            NoteParam::Resonance => note.mod_y = value,
            NoteParam::FinePitch => note.fine_pitch = value,
        }
    }

    fn get_default(self, defaults: &NoteDefaults) -> f64 {
        match self {
            NoteParam::Velocity => defaults.velocity,
            NoteParam::Pan => defaults.pan,
            NoteParam::Release => defaults.release,
            NoteParam::Cutoff => defaults.mod_x,
            NoteParam::Resonance => defaults.mod_y,
            NoteParam::FinePitch => defaults.fine_pitch,
        }
    }

    fn set_default(self, defaults: &mut NoteDefaults, value: f64) {
        match self {
            NoteParam::Velocity => defaults.velocity = value,
            NoteParam::Pan => defaults.pan = value,
            NoteParam::Release => defaults.release = value,
            NoteParam::Cutoff => defaults.mod_x = value,
            NoteParam::Resonance => defaults.mod_y = value,
            NoteParam::FinePitch => defaults.fine_pitch = value,
        }
    }

    /// Mirrors the audio engine's own mapping for cutoff/release
    /// (`audio::mod_x_to_cutoff_hz`/`audio::envelope`'s `release_seconds`)
    /// so the dial's readout always matches what's actually audible —
    /// duplicated rather than shared across the UI/engine boundary, same
    /// trade `widgets::dial`'s own callers already make for their value
    /// text.
    fn value_text(self, value: f64) -> String {
        match self {
            NoteParam::Velocity | NoteParam::Resonance => format!("{:.0}%", value * 100.0),
            NoteParam::Pan => format!("{:+.2}", value - 0.5),
            NoteParam::Release => {
                let r = value.clamp(0.0, 1.0);
                let secs = if r <= 0.5 { (r / 0.5) * 0.05 } else { 0.05 + ((r - 0.5) / 0.5) * (2.0 - 0.05) };
                format!("{:.2}s", secs)
            }
            NoteParam::Cutoff => {
                let hz = 50.0 * (20_000.0_f64 / 50.0).powf(value.clamp(0.0, 1.0));
                format!("{:.0}Hz", hz)
            }
            NoteParam::FinePitch => format!("{:+.0}c", value),
        }
    }
}

/// What a freshly placed note's properties start at, and what the
/// properties panel edits when nothing is selected — Allodium-tauri's own
/// `noteDefaults`.
#[derive(Clone, Copy)]
struct NoteDefaults {
    velocity: f64,
    pan: f64,
    release: f64,
    mod_x: f64,
    mod_y: f64,
    fine_pitch: f64,
    color: u32,
    slide: bool,
}

impl Default for NoteDefaults {
    fn default() -> Self {
        let n = Note::default();
        NoteDefaults { velocity: DEFAULT_VELOCITY, pan: n.pan, release: n.release, mod_x: n.mod_x, mod_y: n.mod_y, fine_pitch: n.fine_pitch, color: n.color, slide: n.slide }
    }
}

#[derive(Clone, Copy)]
struct ParamDrag {
    param: NoteParam,
    start_mouse_y: f32,
    start_value: f64,
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
    /// The active edit tool (`PianoRollTool`'s own doc comment covers what
    /// each one does).
    tool: PianoRollTool,
    /// Note ids the Marquee tool (or a Draw-tool click) has selected.
    /// Driving two things: which notes a group-move drags together
    /// (`DragMode::Moving`'s `initial_positions`), and which notes
    /// Delete/Backspace removes (`delete_selected_or_clear`) — with
    /// nothing selected, that clears every note of the open channel
    /// instead, matching the original's own "Delete with nothing selected"
    /// behavior.
    selected: HashSet<u32>,
    /// Whether the Note Properties panel (`render_note_properties`) is
    /// shown. Minimal compared to the original's panel (velocity only —
    /// the only per-note property this rewrite's `Note` actually has;
    /// pan/release/modX/modY/finePitch/color/slide are all declined, see
    /// this module's own top doc comment), but still toggled the same way.
    show_note_props: bool,
    /// What a freshly placed note's properties start at, and what the
    /// panel edits when nothing is selected — `NoteDefaults`'s own doc
    /// comment.
    defaults: NoteDefaults,
    /// Click-drag state for whichever properties-panel dial is currently
    /// being dragged, same vertical-drag-to-value convention as every
    /// other `dial` in this app (`widgets::dial`'s own doc comment).
    param_drag: Option<ParamDrag>,
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
            tool: PianoRollTool::Draw,
            selected: HashSet::new(),
            // Matches Allodium-tauri's own `PianoRoll.svelte`
            // (`let showNoteProps = $state(true);`) — open by default,
            // not an opt-in the user has to discover via the toolbar
            // toggle first.
            show_note_props: true,
            defaults: NoteDefaults::default(),
            param_drag: None,
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
            // Note ids are only unique within a channel's own content, so
            // a selection built against the previous channel could
            // otherwise land on an unrelated note that happens to share
            // the same id in the newly loaded one.
            self.selected.clear();
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

    fn handle_mouse_down(&mut self, ev: &MouseDownEvent, cx: &mut Context<Self>) {
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
                            self.drag = DragMode::Resizing { note_id: note.id, anchor_step: step, orig_length: note.length };
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

    fn handle_mouse_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
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

    fn end_drag(&mut self, cx: &mut Context<Self>) {
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

    /// Allodium-tauri's own `PianoRollHeader.svelte`: Draw/Delete/Marquee
    /// tool buttons, a Play toggle (solos and plays just the open channel —
    /// `toggle_solo_play`'s own doc comment), and the Note Properties
    /// toggle. The original bundles this with the window's own title/
    /// maximize/close chrome in one `.pr-header`; here that chrome already
    /// comes from `floating_panel.rs` generically for every panel, so this
    /// is just its left-hand controls as their own row.
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_playing = self.channel_rack.read(cx).is_playing();

        div()
            .h(px(28.0))
            .w_full()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .bg(theme::HEADER_BG)
            .border_b_1()
            .border_color(theme::BORDER)
            .child(icon_toggle(
                "pr-tool-draw",
                IconName::Pencil,
                self.tool == PianoRollTool::Draw,
                theme::accent(),
                26.0,
                26.0,
                cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.tool = PianoRollTool::Draw;
                    cx.notify();
                }),
            ))
            .child(icon_toggle(
                "pr-tool-delete",
                IconName::Eraser,
                self.tool == PianoRollTool::Delete,
                theme::accent(),
                26.0,
                26.0,
                cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.tool = PianoRollTool::Delete;
                    cx.notify();
                }),
            ))
            .child(icon_toggle(
                "pr-tool-marquee",
                IconName::SquareDashedMousePointer,
                self.tool == PianoRollTool::Marquee,
                theme::accent(),
                26.0,
                26.0,
                cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.tool = PianoRollTool::Marquee;
                    cx.notify();
                }),
            ))
            .child(div().w(px(1.0)).h(px(16.0)).bg(theme::BORDER))
            .child(icon_toggle(
                "pr-play",
                if is_playing { IconName::Pause } else { IconName::Play },
                is_playing,
                theme::accent(),
                26.0,
                26.0,
                cx.listener(|this, _: &gpui::ClickEvent, _, cx| this.toggle_solo_play(cx)),
            ))
            .child(icon_toggle(
                "pr-note-props",
                IconName::SlidersHorizontal,
                self.show_note_props,
                theme::accent(),
                26.0,
                26.0,
                cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.show_note_props = !this.show_note_props;
                    cx.notify();
                }),
            ))
    }

    /// The Piano Roll's own Play button: solos the open channel for the
    /// duration of playback (`ChannelRack::solo_only`/`restore_solo`)
    /// rather than toggling solo via the row's own mute dot — Allodium-
    /// tauri's `PianoRollHeader.svelte` setting `playback.soloChannelId`
    /// before starting and pressing it again just stops. A no-op with no
    /// channel open.
    fn toggle_solo_play(&mut self, cx: &mut Context<Self>) {
        let Some(channel_id) = self.loaded_channel_id else { return };
        let is_playing = self.channel_rack.read(cx).is_playing();
        self.channel_rack.update(cx, |rack, cx| {
            if is_playing {
                rack.toggle_play(cx);
                rack.restore_solo(cx);
            } else {
                rack.solo_only(channel_id, cx);
                rack.toggle_play(cx);
            }
        });
    }

    /// Note Properties panel: velocity, pan, release, filter cutoff/
    /// resonance, and fine pitch ("pitch bend") dials (`NoteParam`), plus
    /// a color swatch picker and a slide toggle. Edits every selected
    /// note at once when there's a selection; otherwise edits
    /// `self.defaults` (what a freshly placed note gets next), matching
    /// the original's own "edits the selected notes when there are any,
    /// otherwise `noteDefaults`." Mixed-value display (the original's
    /// dimmed "MIXED" label when selected notes disagree) is skipped —
    /// this just shows the first selected note's value for each control.
    /// Docked on the Piano Roll's left, same as Allodium-tauri's own
    /// `.np` (`NotePropertiesPanel.svelte`) — a fixed-width vertical
    /// column (scope label, dials in a 3-wide grid, slide toggle, color
    /// palette), not a header row, placed as the first child of the
    /// piano-keys+grid flex row in `render` rather than above the whole
    /// panel.
    fn render_note_properties(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const PANEL_W: f32 = 138.0;

        let first_selected = self.selected.iter().next().and_then(|id| self.notes.iter().find(|n| n.id == *id));
        let label = if self.selected.is_empty() {
            "New notes".to_string()
        } else {
            format!("{} selected note{}", self.selected.len(), if self.selected.len() == 1 { "" } else { "s" })
        };
        let selected_color = first_selected.map(|n| n.color).unwrap_or(self.defaults.color);
        let selected_slide = first_selected.map(|n| n.slide).unwrap_or(self.defaults.slide);
        let this_entity = cx.entity();

        let dial_cell = |this: &Self, cx: &mut Context<Self>, param: NoteParam| {
            let value = first_selected.map(|n| param.get(n)).unwrap_or_else(|| param.get_default(&this.defaults));
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(crate::widgets::dial(
                    ("pr-param", param as usize),
                    param.name(),
                    param.value_text(value),
                    value - param.min(),
                    param.max() - param.min(),
                    210.0,
                    300.0,
                    20.0,
                    cx.listener(move |this, ev: &MouseDownEvent, _, _| this.begin_param_drag(param, f32::from(ev.position.y))),
                    {
                        let entity = this_entity.clone();
                        move |_: &mut Window, cx: &mut gpui::App| entity.update(cx, |pr, cx| pr.set_param(param, param.reset_value(), cx))
                    },
                    cx.listener(|_this, _ev: &(bool, &'static str, String), _, _| {}),
                ))
                .child(div().text_size(px(8.0)).text_color(theme::BTN_TEXT).child(param.name()))
        };

        let mut dial_rows = div().flex().flex_col().gap_2();
        for chunk in NoteParam::ALL.chunks(3) {
            let mut dial_row = div().flex().justify_between();
            for param in chunk {
                dial_row = dial_row.child(dial_cell(self, cx, *param));
            }
            dial_rows = dial_rows.child(dial_row);
        }

        div()
            .id("note-properties")
            .w(px(PANEL_W))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_3()
            .p_2()
            .overflow_y_scroll()
            .bg(theme::HEADER_BG)
            .border_r_1()
            .border_color(theme::BORDER)
            .child(
                div()
                    .text_size(px(10.0))
                    .text_center()
                    .text_color(if self.selected.is_empty() { theme::BTN_TEXT } else { theme::accent() })
                    .child(label),
            )
            .child(dial_rows)
            .child(
                div()
                    .id("pr-slide")
                    .w_full()
                    .h(px(26.0))
                    .rounded(theme::RADIUS_SM)
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .cursor_pointer()
                    .text_size(px(11.0))
                    .border_1()
                    .text_color(if selected_slide { theme::accent() } else { theme::BTN_TEXT })
                    .border_color(if selected_slide { theme::accent().alpha(0.4) } else { theme::BTN_BORDER })
                    .when(selected_slide, |d| d.bg(theme::accent().alpha(0.1)))
                    .hover(|s| if selected_slide { s } else { s.bg(theme::BTN_HOVER).text_color(theme::MAIN_TEXT) })
                    .child(Icon::new(IconName::TrendingUp).small())
                    .child("Slide")
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| this.set_slide(!selected_slide, cx))),
            )
            .child({
                let swatches = note_color_palette().into_iter().enumerate().map(|(i, rgb)| {
                    let active = selected_color == rgb;
                    div()
                        .id(("pr-color", i))
                        .w(px(16.0))
                        .h(px(16.0))
                        .rounded(theme::RADIUS_XS)
                        .bg(gpui::rgb(rgb))
                        .cursor_pointer()
                        .when(active, |d| d.border_2().border_color(theme::MAIN_TEXT))
                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| this.set_color(rgb, cx)))
                        .into_any_element()
                });
                let none_swatch = {
                    let active = selected_color == 0;
                    div()
                        .id("pr-color-none")
                        .w(px(16.0))
                        .h(px(16.0))
                        .rounded(theme::RADIUS_XS)
                        .border_1()
                        .border_color(theme::BTN_BORDER)
                        .cursor_pointer()
                        .when(active, |d| d.border_2().border_color(theme::MAIN_TEXT))
                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| this.set_color(0, cx)))
                        .into_any_element()
                };
                div().flex().flex_wrap().gap_1().justify_center().children(swatches.chain(std::iter::once(none_swatch)))
            })
    }

    fn begin_param_drag(&mut self, param: NoteParam, start_mouse_y: f32) {
        let start_value =
            self.selected.iter().next().and_then(|id| self.notes.iter().find(|n| n.id == *id)).map(|n| param.get(n)).unwrap_or_else(|| param.get_default(&self.defaults));
        self.param_drag = Some(ParamDrag { param, start_mouse_y, start_value });
    }

    fn update_param_drag(&mut self, mouse_y: f32, cx: &mut Context<Self>) {
        let Some(drag) = self.param_drag else { return };
        const PARAM_DRAG_SENSITIVITY: f32 = 150.0;
        let dy = drag.start_mouse_y - mouse_y;
        let span = drag.param.max() - drag.param.min();
        let value = (drag.start_value + (dy / PARAM_DRAG_SENSITIVITY) as f64 * span).clamp(drag.param.min(), drag.param.max());
        self.set_param(drag.param, value, cx);
    }

    fn end_param_drag(&mut self) {
        self.param_drag = None;
    }

    /// Writes `value` to every selected note's `param`, or to
    /// `self.defaults` (what the *next* placed note gets) with nothing
    /// selected — every properties-panel dial drives this, same split as
    /// the original's "edits the selected notes when there are any,
    /// otherwise `noteDefaults`."
    fn set_param(&mut self, param: NoteParam, value: f64, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            param.set_default(&mut self.defaults, value);
            cx.notify();
            return;
        }
        for note in self.notes.iter_mut().filter(|n| self.selected.contains(&n.id)) {
            param.set(note, value);
        }
        let Some(channel_id) = self.loaded_channel_id else { return };
        self.commit_live(channel_id, cx);
    }

    fn set_color(&mut self, rgb: u32, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.defaults.color = rgb;
            cx.notify();
            return;
        }
        for note in self.notes.iter_mut().filter(|n| self.selected.contains(&n.id)) {
            note.color = rgb;
        }
        let Some(channel_id) = self.loaded_channel_id else { return };
        self.commit_live(channel_id, cx);
    }

    fn set_slide(&mut self, slide: bool, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.defaults.slide = slide;
            cx.notify();
            return;
        }
        for note in self.notes.iter_mut().filter(|n| self.selected.contains(&n.id)) {
            note.slide = slide;
        }
        let Some(channel_id) = self.loaded_channel_id else { return };
        self.commit_live(channel_id, cx);
    }
}

/// A fixed palette of evenly-spaced hues for the properties panel's color
/// swatches — Allodium-tauri's own fixed 15-color note palette, just
/// generated instead of hand-picked (same `hsl_to_rgb_u32` the Channel
/// Rack's "Recolor randomly" already uses).
fn note_color_palette() -> Vec<u32> {
    const COUNT: usize = 10;
    (0..COUNT).map(|i| crate::color_utils::hsl_to_rgb_u32(i as f32 / COUNT as f32, 0.55, 0.55)).collect()
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
        let selected = self.selected.clone();
        let dying: Vec<(Note, f32)> = self
            .dying
            .iter()
            .map(|d| (d.note.clone(), d.died_at.elapsed().as_secs_f32() / DEATH_FADE_SECS))
            .collect();
        let scroll_x = self.scroll_x;
        let scroll_y = self.scroll_y;
        let grid_origin = self.grid_origin.clone();
        let grid_size = self.grid_size.clone();

        // The marquee box itself, in grid-content pixels (scroll already
        // subtracted) — drawn as a plain absolute-positioned overlay
        // rather than through `paint_grid`'s canvas, since it's simple
        // enough not to need virtualization and this avoids threading yet
        // another parameter through that closure.
        let marquee_rect = if let DragMode::Marquee { anchor_step, anchor_pitch, cur_step, cur_pitch, .. } = &self.drag {
            let (anchor_step, anchor_pitch, cur_step, cur_pitch) = (*anchor_step, *anchor_pitch, *cur_step, *cur_pitch);
            let (lo_step, hi_step) = (anchor_step.min(cur_step), anchor_step.max(cur_step));
            let (lo_pitch, hi_pitch) = (anchor_pitch.min(cur_pitch), anchor_pitch.max(cur_pitch));
            let x = step_to_x(lo_step) - scroll_x;
            let w = (step_to_x(hi_step + 1.0) - step_to_x(lo_step)).max(1.0);
            let y = pitch_to_y(hi_pitch) - scroll_y;
            let h = (pitch_to_y(lo_pitch) + KEY_H - pitch_to_y(hi_pitch)).max(1.0);
            Some((x, y, w, h))
        } else {
            None
        };

        // Cursor affordance: an active drag wins outright; otherwise it
        // reflects the idle hover classification `handle_mouse_move`'s
        // passive branch below keeps up to date. This is what makes the
        // resize zone this stage adds actually discoverable rather than
        // a hitbox nobody would find by accident.
        let mut grid_area = div()
            .relative()
            .size_full()
            .child(
                canvas(
                    move |_, _, _| {},
                    move |bounds, _, window, _| {
                        grid_origin.set(bounds.origin);
                        grid_size.set(bounds.size);
                        paint_grid(window, bounds, scroll_x, scroll_y, &notes, &selected, &background_notes, &dying, current_step);
                    },
                )
                .size_full(),
            );
        if let Some((x, y, w, h)) = marquee_rect {
            grid_area = grid_area.child(
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .h(px(h))
                    .border_1()
                    .border_color(theme::accent())
                    .bg(theme::accent().alpha(0.12)),
            );
        }
        let grid_area = grid_area
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
        let grid_area = match &self.drag {
            DragMode::Moving { .. } => grid_area.cursor_grabbing(),
            DragMode::Resizing { .. } => grid_area.cursor_col_resize(),
            DragMode::Deleting => grid_area.cursor_not_allowed(),
            DragMode::Marquee { .. } => grid_area.cursor_crosshair(),
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
            // Whichever properties-panel dial is being dragged
            // (`render_note_properties`) tracks here rather than on the
            // small dial itself, same reason every other dial-driving
            // container in this app (e.g. `ChannelRack`'s own root div)
            // does it at this level: a drag has to keep updating even
            // once the cursor leaves the dial's own tiny hitbox.
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if this.param_drag.is_some() {
                    this.update_param_drag(f32::from(ev.position.y), cx);
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| this.end_param_drag()))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| this.end_param_drag()))
            .child(self.render_toolbar(cx))
            .child(self.render_channel_picker(cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .w_full()
                    .flex()
                    .overflow_hidden()
                    .when(self.show_note_props, |d| d.child(self.render_note_properties(cx)))
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
    selected: &HashSet<u32>,
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
        paint_note(window, origin, scroll_x, scroll_y, note, ghost_color, 1.0, 0.0, gpui::transparent_black());
    }

    // Each note draws in its own `Note::color` (falling back to the
    // channel's accent when unset). While a selection exists, selected
    // notes additionally get a bright rim (standing in for the original's
    // white-rim/glow/diagonal-stripe treatment — no stripe-drawing
    // primitive here, see this module's own top doc comment on declined
    // fidelity) and everything unselected dims to 40%.
    for note in notes {
        if note.start + note.length <= visible_start || note.start > visible_end {
            continue;
        }
        if note.pitch < first_visible_pitch - 1 || note.pitch > last_visible_pitch + 1 {
            continue;
        }
        let base_color: Hsla = if note.color != 0 { gpui::rgb(note.color).into() } else { theme::accent() };
        let is_selected = selected.contains(&note.id);
        let note_color = if selected.is_empty() || is_selected { base_color } else { base_color.alpha(0.4) };
        let (border_width, border_color) = if is_selected { (1.5, theme::MAIN_TEXT) } else { (0.0, gpui::transparent_black()) };
        paint_note(window, origin, scroll_x, scroll_y, note, note_color, 1.0, border_width, border_color);
    }

    // Dying notes: a brief flash that fades and shrinks slightly.
    for (note, progress) in dying {
        let alpha = (1.0 - progress).max(0.0);
        let mut color = theme::warm_accent();
        color.a = alpha;
        paint_note(window, origin, scroll_x, scroll_y, note, color, alpha, 0.0, gpui::transparent_black());
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

#[allow(clippy::too_many_arguments)]
fn paint_note(
    window: &mut Window,
    origin: Point<gpui::Pixels>,
    scroll_x: f32,
    scroll_y: f32,
    note: &Note,
    color: Hsla,
    scale: f32,
    border_width: f32,
    border_color: Hsla,
) {
    let x = origin.x + px(step_to_x(note.start) - scroll_x + 0.5);
    let y = origin.y + px(pitch_to_y(note.pitch) - scroll_y + 0.5);
    let w = (note.length as f32 * STEP_W - 1.0).max(1.0) * scale;
    let h = (KEY_H - 1.0).max(1.0) * scale;
    window.paint_quad(quad(
        gpui::Bounds { origin: point(x, y), size: size(px(w), px(h)) },
        px(2.0),
        color,
        px(border_width),
        border_color,
        Default::default(),
    ));
}
