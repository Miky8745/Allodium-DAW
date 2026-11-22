//! All custom canvas painting: the piano-keys sidebar and the note
//! grid's own virtualized rendering (row striping, beat-group banding,
//! notes, dying-note flash, the playhead overlay). Nothing here owns any
//! state of its own — every function takes exactly what it needs to draw
//! one frame, so none of this needs `PianoRoll` at all.

use std::collections::HashSet;

use gpui::{
    Hsla, IntoElement, ParentElement, Point, Styled, Window, div, linear_color_stop, linear_gradient, point, prelude::*, px, quad, size,
};

use crate::project::Note;
use crate::theme;

use super::{KEY_H, PITCH_COUNT, STEP_W, pitch_to_y, step_to_x};

const KEY_COL_W: f32 = 56.0;

fn is_black_key(pitch: i32) -> bool {
    matches!(pitch.rem_euclid(12), 1 | 3 | 6 | 8 | 10)
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
pub(super) fn piano_keys(scroll_y: f32, held: &HashSet<i32>) -> impl IntoElement {
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
        .flex_1()
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
pub(super) fn paint_grid(
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
    // primitive here, see `piano_roll.rs`'s own top doc comment on
    // declined fidelity) and everything unselected dims to 40%.
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
