use gpui::component::menu::{ContextMenuExt, PopupMenuItem};
use gpui::{App, ClickEvent, IntoElement, MouseButton, MouseDownEvent, ParentElement, Styled, Window, div, prelude::*, px};

use crate::theme;

/// A circular pot-style control — Allodium-tauri's own `Dial.svelte`,
/// minus the SVG arc stroke (this codebase has no path-drawing primitive
/// for an arbitrary arc; see `docs/gpui-api-notes.md`'s "no rotation
/// transform" note for the same limitation elsewhere). The indicator dot's
/// position is still the real trig the original computes, just drawn
/// without the colored sweep behind it — close enough to read as "a dial
/// at roughly this angle" without faking an arc with rectangles.
///
/// `on_drag_start` should kick off a vertical-drag value change (the
/// original's own convention: distance from the mouse-down point maps
/// linearly to `value`, scaled by `max`); `on_reset` resets to a
/// caller-chosen default and fires on both a double-click and "Reset to
/// default" in the right-click menu, labeled with `name`. On hover, `name`
/// and `value_text` are bundled into `on_hover` — typically pushed
/// straight into wherever the app's own hover-info readout lives (e.g. the
/// toolbar's), same place every other hoverable control already reports
/// to. None of drag/reset/hover-sink behavior is built in here: this
/// widget only draws the dial, wires its events, and bundles the info —
/// what actually happens with it stays with whichever entity owns the
/// value.
#[allow(clippy::too_many_arguments)]
pub fn dial(
    id: impl Into<gpui::ElementId>,
    name: &'static str,
    value_text: impl Into<String>,
    value: f64,
    max: f64,
    sweep_start: f32,
    sweep_degrees: f32,
    size: f32,
    on_drag_start: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_reset: impl Fn(&mut Window, &mut App) + Clone + 'static,
    on_hover: impl Fn(&(bool, &'static str, String), &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let angle_deg = sweep_start + (value / max).clamp(0.0, 1.0) as f32 * sweep_degrees;
    let angle_rad = (angle_deg - 90.0) * std::f32::consts::PI / 180.0;
    let radius = size / 2.0 - 3.0;
    let center = size / 2.0;
    const DOT: f32 = 4.0;
    let dot_x = center + radius * angle_rad.cos() - DOT / 2.0;
    let dot_y = center + radius * angle_rad.sin() - DOT / 2.0;
    let value_text = value_text.into();
    let on_reset_dbl_click = on_reset.clone();

    div()
        .id(id)
        .relative()
        .w(px(size))
        .h(px(size))
        .rounded_full()
        .bg(theme::BG_PANEL)
        .border_1()
        .border_color(theme::BORDER)
        .cursor_default()
        .child(div().absolute().left(px(dot_x)).top(px(dot_y)).w(px(DOT)).h(px(DOT)).rounded_full().bg(theme::accent()))
        .on_mouse_down(MouseButton::Left, on_drag_start)
        .on_click(move |ev: &ClickEvent, window, cx| {
            if ev.click_count() >= 2 {
                on_reset_dbl_click(window, cx);
            }
        })
        .on_hover(move |hovering: &bool, window, cx| {
            on_hover(&(*hovering, name, value_text.clone()), window, cx);
        })
        .context_menu(move |menu, _, _| {
            let on_reset = on_reset.clone();
            menu.item(PopupMenuItem::new(format!("Reset {name}")).on_click(move |_, window, cx| on_reset(window, cx)))
        })
}
