use gpui::{App, Div, ParentElement, ScrollEvent, Stateful, Styled, Window, div, prelude::*, px};

use crate::theme;

/// A narrow number readout that steps on scroll — Allodium-tauri's own
/// `ScrollField.svelte` (time signature, pattern length, mixer-track
/// routing), minus its own click-drag-to-scrub gesture (scroll alone
/// covers the same range adjustment with far less code, and every other
/// control already using this reads a scroll or a dedicated `dial` drag,
/// not a third input convention).
pub fn scroll_field(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
    width: f32,
    on_scroll: impl Fn(&ScrollEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(width))
        .h(px(18.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::RADIUS_XS)
        .bg(theme::BG_PANEL)
        .border_1()
        .border_color(theme::BORDER)
        .text_size(px(10.0))
        .text_color(theme::accent_dim())
        .cursor_default()
        .hover(|s| s.bg(theme::BTN_HOVER))
        .child(text.into())
        .on_scroll(on_scroll)
}
