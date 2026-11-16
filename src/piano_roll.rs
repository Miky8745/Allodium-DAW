//! Piano Roll: the virtualized note grid, ported from
//! `Allodium-GPUI-test/src/grid.rs` + `src/model.rs` (see
//! `../03-reuse-map.md`) and wired to a real channel's real note data
//! instead of a synthetic seeded `Vec<Note>`. See
//! `docs/stage-05-piano-roll.md`.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use gpui::{
    Context, Entity, Hsla, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, Render, ScrollEvent, Styled, Window, canvas, div, point,
    prelude::*, px, quad, rgb, size,
};

use crate::channel_rack::ChannelRack;
use crate::project::Note;
use crate::theme;

const PITCH_COUNT: i32 = 128;
const KEY_H: f32 = 18.0;
const STEP_W: f32 = 24.0;
const DEFAULT_NOTE_LEN: f64 = 8.0;
const DEFAULT_VELOCITY: f64 = 1.0;
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
    /// Right-click-drag delete: remove every note the cursor passes over.
    Deleting,
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
    dying: Vec<DyingNote>,
    /// The grid canvas's own on-screen origin, captured from its `canvas`
    /// paint closure each frame. Plain `div()`s don't hand their own
    /// screen bounds to mouse-event handlers (only `canvas`'s prepaint/
    /// paint closures get a `bounds` param — see `docs/gpui-api-notes.md`),
    /// and unlike the prototype this grid no longer fills the whole
    /// window (it sits below the toolbar/Channel Rack/its own header), so
    /// `event.position` (window-content coordinates) needs this subtracted
    /// before it maps onto grid-local pixels.
    grid_origin: Rc<Cell<Point<Pixels>>>,
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
            dying: Vec::new(),
            grid_origin: Rc::new(Cell::new(point(px(0.0), px(0.0)))),
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
        let origin = self.grid_origin.get();
        let x = f32::from(position.x - origin.x) + self.scroll_x;
        let y = f32::from(position.y - origin.y) + self.scroll_y;
        (x_to_step(x), y_to_pitch(y))
    }

    fn handle_mouse_down(&mut self, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        let Some(channel_id) = self.loaded_channel_id else { return };
        let (step, pitch) = self.pointer_to_grid(ev.position);

        match ev.button {
            MouseButton::Right => {
                self.drag = DragMode::Deleting;
                self.delete_at(step, pitch);
            }
            MouseButton::Left => {
                if let Some(idx) = self.note_at(step, pitch) {
                    let note = self.notes[idx].clone();
                    self.drag = DragMode::Moving {
                        note_id: note.id,
                        grab_offset_steps: step - note.start,
                        grab_offset_pitch: pitch - note.pitch,
                    };
                } else {
                    let max_start = (self.pattern_length_steps as f64 - DEFAULT_NOTE_LEN).max(0.0);
                    let start = step.min(max_start);
                    let note = Note {
                        id: next_note_id(&self.notes),
                        pitch,
                        start,
                        length: DEFAULT_NOTE_LEN,
                        velocity: DEFAULT_VELOCITY,
                    };
                    let note_id = note.id;
                    self.notes.push(note);
                    self.drag = DragMode::Moving {
                        note_id,
                        grab_offset_steps: 0.0,
                        grab_offset_pitch: 0,
                    };
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
            DragMode::Deleting => {
                self.delete_at(step, pitch);
                self.commit_live(channel_id, cx);
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
        self.channel_rack.update(cx, |rack, cx| rack.update_notes(channel_id, notes, cx));
    }
}

impl Render for PianoRoll {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_selection(cx);
        self.clamp_scroll(window.viewport_size());
        self.prune_dying();
        window.request_animation_frame();

        let rack = self.channel_rack.read(cx);
        let playing = rack.is_playing();
        let current_step = if playing { rack.current_step() } else { -1.0 };

        let notes = self.notes.clone();
        let dying: Vec<(Note, f32)> = self
            .dying
            .iter()
            .map(|d| (d.note.clone(), d.died_at.elapsed().as_secs_f32() / DEATH_FADE_SECS))
            .collect();
        let scroll_x = self.scroll_x;
        let scroll_y = self.scroll_y;
        let grid_origin = self.grid_origin.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::BG_APP)
            .child(
                div()
                    .size_full()
                    .overflow_hidden()
                    .bg(rgb(0x1a1a1a))
                    // Rounds its own bottom corners to match the floating
                    // panel's outer rounded edge — GPUI's overflow clip is
                    // a plain rectangle, so without this, this square-
                    // cornered full-size background would show past the
                    // panel's rounded corner instead of being clipped to
                    // it (see `floating_panel.rs`'s own note on the same
                    // limitation).
                    .rounded_b(theme::RADIUS_MD)
                    .on_scroll(cx.listener(|this, ev: &ScrollEvent, window, cx| {
                        let delta = ev.delta.pixel_delta(px(KEY_H));
                        this.scroll_x -= f32::from(delta.x);
                        this.scroll_y -= f32::from(delta.y);
                        this.clamp_scroll(window.viewport_size());
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size_full()
                            .child(
                                canvas(
                                    move |_, _, _| {},
                                    move |bounds, _, window, _| {
                                        grid_origin.set(bounds.origin);
                                        paint_grid(window, bounds, scroll_x, scroll_y, &notes, &dying, current_step);
                                    },
                                )
                                .size_full(),
                            )
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
            Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.03 },
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
            Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.25 },
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

    let note_color = rgb(0x4fa8e8);
    for note in notes {
        if note.start + note.length <= visible_start || note.start > visible_end {
            continue;
        }
        if note.pitch < first_visible_pitch - 1 || note.pitch > last_visible_pitch + 1 {
            continue;
        }
        paint_note(window, origin, scroll_x, scroll_y, note, note_color.into(), 1.0);
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
                Hsla { h: 0.0, s: 0.8, l: 0.6, a: 0.9 },
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
