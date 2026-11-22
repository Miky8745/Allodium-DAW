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
//!   buttons (`toolbar::PianoRollTool`), a Play button that solos and
//!   plays just the open channel (`toolbar::toggle_solo_play`), and a
//!   Note Properties toggle. Marquee is a real box-select
//!   (`interaction::DragMode::Marquee`, `update_marquee_selection`) with
//!   Ctrl+drag available in every tool, group-move for a multi-note
//!   selection, and Delete/Backspace/Escape routed in from `AppShell`'s
//!   key dispatcher (`shell.rs`'s `route_piano_roll_key`) while this
//!   panel is open and frontmost.
//! - **Added** (same later pass, extended further at the user's explicit
//!   request): the rest of the original's `NotePropertiesPanel` —
//!   `Note` gained `pan`/`release`/`mod_x`/`mod_y`/`fine_pitch`/`color`/
//!   `slide` (`src/project.rs`, all `#[serde(default)]` so older saves
//!   keep loading), the properties panel (`properties::render_note_
//!   properties`) edits all of them via `NoteParam` + a color swatch
//!   picker + a slide toggle, and notes draw in their own color in the
//!   grid (`paint::paint_grid`'s own note loop) with a bright rim
//!   standing in for the original's white-rim/glow/diagonal-stripe
//!   selected-note treatment (no stripe-drawing primitive here). The
//!   audio engine (`src/audio/mod.rs`) actually applies all of these
//!   now, not just storing them: `trigger_channel_step` offsets the
//!   channel's own pan by `note.pan`, shifts playback rate by
//!   `note.fine_pitch` cents (`cents_to_rate_multiplier`), runs a
//!   per-voice RBJ lowpass biquad from `mod_x`/`mod_y` (`BiquadLowpass`,
//!   bypassed entirely at the default "fully open" cutoff so the common
//!   case costs nothing extra per sample), extends the envelope's
//!   release tail past the note's own end for `release` values above
//!   the `0.5` default (`Envelope::shaped_with_release`), and — a real
//!   engine change, not just the Piano Roll — a `slide` note never
//!   starts a voice of its own; instead it bends every still-ringing
//!   voice of the same channel and note color to its own pitch over its
//!   own length (`Voice::begin_slide`/`ChannelVoiceData::id`).
//!   Mixed-value display (the original's dimmed "MIXED" label when
//!   selected notes disagree) is skipped — the panel just shows the
//!   first selected note's value per control.
//! - **Added** (yet another later pass, at the user's explicit request):
//!   the Playlist's own numbered ruler (`scroll::render_beat_ruler` —
//!   bars, same as the Playlist's own numbering, on top of this grid's
//!   existing per-beat light/dark banding rather than replacing it), a
//!   draggable horizontal scrollbar (`scroll::render_h_scrollbar`/
//!   `HScrollGeometry`, hand-built since the grid's content is painted
//!   procedurally in a `canvas` rather than laid out as real DOM
//!   `ScrollHandle` could track), and the overview/minimap strip from
//!   the list below (`scroll::render_overview`) — now implemented after
//!   all, sharing its viewport-window geometry with the scrollbar via
//!   `HScrollGeometry` rather than being a declined stretch item; see
//!   the next bullet for what's still actually out.
//! - **Declined, matching `02-feature-scope.md`'s own stretch list**:
//!   zoom, copy/paste, the velocity lane, and per-channel independent-
//!   loop playhead wrapping. Also declined:
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
//!
//! Split into `piano_roll/*.rs` (mouse/drag interaction, the toolbar +
//! channel picker, the Note Properties sidebar, horizontal-nav
//! scrolling, and canvas painting) the same way `channel_rack.rs` was —
//! this file is now just the shared state (`PianoRoll`'s own fields),
//! construction, the shared step/pitch pixel math every submodule needs,
//! and the top-level `render()` that assembles their pieces.

mod interaction;
mod paint;
mod properties;
mod scroll;
mod toolbar;

use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

use gpui::{
    Context, Entity, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollEvent,
    Styled, Window, canvas, div, point, prelude::*, px, size,
};

use crate::channel_rack::ChannelRack;
use crate::project::Note;
use crate::theme;

use interaction::{DragMode, DyingNote, Hover};
use properties::{NoteDefaults, ParamDrag};
use scroll::HScrollDrag;
use toolbar::PianoRollTool;

const PITCH_COUNT: i32 = 128;
const KEY_H: f32 = 18.0;
const STEP_W: f32 = 24.0;
const DEFAULT_NOTE_LEN: f64 = 8.0;
const DEATH_FADE_SECS: f32 = 0.3;

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
    /// The active edit tool (`toolbar::PianoRollTool`'s own doc comment
    /// covers what each one does).
    tool: PianoRollTool,
    /// Note ids the Marquee tool (or a Draw-tool click) has selected.
    /// Driving two things: which notes a group-move drags together
    /// (`DragMode::Moving`'s `initial_positions`), and which notes
    /// Delete/Backspace removes (`delete_selected_or_clear`) — with
    /// nothing selected, that clears every note of the open channel
    /// instead, matching the original's own "Delete with nothing selected"
    /// behavior.
    selected: HashSet<u32>,
    /// Whether the Note Properties panel (`properties::render_note_
    /// properties`) is shown.
    show_note_props: bool,
    /// What a freshly placed note's properties start at, and what the
    /// panel edits when nothing is selected — `NoteDefaults`'s own doc
    /// comment.
    defaults: NoteDefaults,
    /// Click-drag state for whichever properties-panel dial is currently
    /// being dragged, same vertical-drag-to-value convention as every
    /// other `dial` in this app (`widgets::dial`'s own doc comment).
    param_drag: Option<ParamDrag>,
    h_scroll_drag: Option<HScrollDrag>,
    dying: Vec<DyingNote>,
    /// The pitch a note click lit up and sounded via
    /// `interaction::begin_live_preview`, remembered so `end_drag`/mouse-up
    /// knows which key to release — see that function's own doc comment.
    live_preview_pitch: Option<i32>,
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
            h_scroll_drag: None,
            dying: Vec::new(),
            live_preview_pitch: None,
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
        // rather than through `paint::paint_grid`'s canvas, since it's
        // simple enough not to need virtualization and this avoids
        // threading yet another parameter through that closure.
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
                        paint::paint_grid(window, bounds, scroll_x, scroll_y, &notes, &selected, &background_notes, &dying, current_step);
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
            // (`properties::render_note_properties`) tracks here rather
            // than on the small dial itself, same reason every other
            // dial-driving container in this app (e.g. `ChannelRack`'s
            // own root div) does it at this level: a drag has to keep
            // updating even once the cursor leaves the dial's own tiny
            // hitbox.
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if this.param_drag.is_some() {
                    this.update_param_drag(f32::from(ev.position.y), cx);
                }
                if this.h_scroll_drag.is_some() {
                    this.update_h_scroll_drag(f32::from(ev.position.x), cx);
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| {
                this.end_param_drag();
                this.end_h_scroll_drag();
            }))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| {
                this.end_param_drag();
                this.end_h_scroll_drag();
            }))
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
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .h_full()
                            .flex_shrink_0()
                            // The keys sidebar scrolls in lockstep with the
                            // grid's rows, but the grid sits below the
                            // overview strip + bar ruler while this column
                            // fills the whole panel height from the top —
                            // without reserving that same height here first,
                            // every key row renders `OVERVIEW_H + RULER_H`
                            // (44px, ~2.4 rows) higher than the grid row it's
                            // supposed to label, making notes look rendered
                            // too low relative to the keys beside them.
                            .child(div().h(px(scroll::OVERVIEW_H + scroll::RULER_H)).w_full().flex_shrink_0().bg(theme::HEADER_BG))
                            .child(paint::piano_keys(self.scroll_y, self.channel_rack.read(cx).held_pitches())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(self.render_overview(cx))
                            .child(self.render_beat_ruler(cx))
                            .child(
                                div()
                                    .flex_1()
                                    .w_full()
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
                            )
                            .child(self.render_h_scrollbar(cx)),
                    ),
            )
    }
}
