//! Generic floating-window chrome shared by the four panels (Channel
//! Rack, Piano Roll, Playlist, Mixer). None of those views know they're
//! floating — this module just wraps whatever they render in a
//! draggable/resizable/closable/focusable frame. See
//! `docs/stage-08-visual-redesign.md` for the design rationale (niri's
//! own flat, border-only-when-focused window convention, deliberately
//! not the original app's drop-shadow "SaaS card" look).

use gpui::{
    App, ClickEvent, IntoElement, MouseButton, MouseDownEvent, ParentElement, Pixels, Point,
    SharedString, Size, Styled, Window, div, prelude::*, px,
};

use crate::theme;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PanelKind {
    ChannelRack,
    PianoRoll,
    Playlist,
    Mixer,
}

impl PanelKind {
    pub const ALL: [PanelKind; 4] = [PanelKind::ChannelRack, PanelKind::PianoRoll, PanelKind::Playlist, PanelKind::Mixer];

    /// A plain Unicode symbol, not a Nerd-Font-specific PUA glyph — safe
    /// in any font (including a fallback if the exact Nerd Font variant
    /// somehow isn't resolved), while still giving the toolbar/title bars
    /// the icon-plus-label idiom waybar's own modules use.
    pub fn glyph(self) -> &'static str {
        match self {
            PanelKind::ChannelRack => "▦",
            PanelKind::PianoRoll => "♪",
            PanelKind::Playlist => "☰",
            PanelKind::Mixer => "▬",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PanelKind::ChannelRack => "Channel Rack",
            PanelKind::PianoRoll => "Piano Roll",
            PanelKind::Playlist => "Playlist",
            PanelKind::Mixer => "Mixer",
        }
    }
}

#[derive(Clone, Copy)]
pub struct PanelGeometry {
    pub position: Point<Pixels>,
    pub size: Size<Pixels>,
    pub open: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum PanelDragKind {
    Move,
    Resize,
}

#[derive(Clone, Copy)]
pub struct PanelDrag {
    pub kind: PanelDragKind,
    pub panel: PanelKind,
    pub start_mouse: Point<Pixels>,
    pub start_position: Point<Pixels>,
    pub start_size: Size<Pixels>,
}

pub const MIN_PANEL_SIZE: Size<Pixels> = Size { width: px(280.0), height: px(160.0) };
const TITLE_BAR_H: Pixels = px(26.0);
const RESIZE_HANDLE_SIZE: Pixels = px(14.0);

/// Wraps `body` in the shared panel chrome: a title bar (glyph + title +
/// close button, draggable to move the panel) and a resize handle at the
/// bottom-right corner. `on_focus` fires on a click anywhere in the panel
/// (title bar or body) — confirmed against GPUI's own dispatch (`div.rs`)
/// that a parent's `on_any_mouse_down` still fires regardless of what a
/// child's own mouse handler does, so this doesn't interfere with the
/// Piano Roll grid's or a slider's own click handling underneath.
pub fn render_panel(
    kind: PanelKind,
    title: impl Into<SharedString>,
    geometry: PanelGeometry,
    focused: bool,
    dragging_this: bool,
    body: impl IntoElement,
    on_focus: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_drag_start: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_resize_start: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .absolute()
        .left(geometry.position.x)
        .top(geometry.position.y)
        .w(geometry.size.width)
        .h(geometry.size.height)
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(theme::RADIUS_MD)
        .bg(theme::BG_PANEL)
        // A faint rest-state shadow — the thing that actually reads as
        // "flat ImGUI chrome" wasn't the lack of a border, it was the
        // total absence of any depth cue at rest. Kept deliberately
        // faint (not the generic uniform "SaaS card" shadow) and still
        // distinct from the heavier one that appears while dragging.
        .shadow_xs()
        .when(focused, |d| d.border(theme::FOCUS_RING_WIDTH).border_color(theme::FOCUS_RING))
        .when(dragging_this, |d| d.shadow_lg())
        .on_any_mouse_down(on_focus)
        .child(
            div()
                .id(("panel-titlebar", kind as u8 as usize))
                .h(TITLE_BAR_H)
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .bg(theme::BG_PILL)
                .cursor_default()
                .on_mouse_down(MouseButton::Left, on_drag_start)
                .child(div().text_size(px(11.0)).child(kind.glyph()))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme::ACCENT_DIM)
                        .child(title.into()),
                )
                .child(
                    div()
                        .id(("panel-close", kind as u8 as usize))
                        .w(px(18.0))
                        .h(px(18.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(theme::RADIUS_XS)
                        .text_color(theme::ACCENT_DIM)
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::DESTRUCTIVE).text_color(theme::ACCENT_TEXT))
                        .child("×")
                        .on_click(on_close),
                ),
        )
        .child(div().flex_1().w_full().overflow_hidden().child(body))
        .child(
            div()
                .id(("panel-resize", kind as u8 as usize))
                .absolute()
                .right_0()
                .bottom_0()
                .w(RESIZE_HANDLE_SIZE)
                .h(RESIZE_HANDLE_SIZE)
                .flex()
                .items_end()
                .justify_end()
                .px(px(2.0))
                .pb(px(1.0))
                .cursor_default()
                .text_size(px(9.0))
                .text_color(theme::ACCENT_DIM.alpha(0.35))
                .hover(|s| s.text_color(theme::ACCENT_DIM))
                .child("⋰")
                .on_mouse_down(MouseButton::Left, on_resize_start),
        )
}
