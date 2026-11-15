//! Generic floating-window chrome shared by the four panels (Channel
//! Rack, Piano Roll, Playlist, Mixer). None of those views know they're
//! floating — this module just wraps whatever they render in a
//! draggable/resizable/closable/focusable frame. See
//! `docs/stage-08-visual-redesign.md` for the design rationale (niri's
//! own flat, border-only-when-focused window convention, deliberately
//! not the original app's drop-shadow "SaaS card" look).

use gpui::assets::IconName;
use gpui::component::{Icon, Sizable};
use gpui::{
    App, BoxShadow, ClickEvent, Hsla, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    Pixels, Point, SharedString, Size, Styled, Window, div, point, prelude::*, px,
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

    /// A real vector icon (the bundled Lucide catalog — see
    /// `gpui::assets::AllAssets` registered in `main.rs`) rather than a
    /// Unicode-glyph stand-in, so the toolbar/title bars draw one
    /// consistent icon family instead of a grab-bag of text approximations.
    pub fn icon(self) -> IconName {
        match self {
            PanelKind::ChannelRack => IconName::Grid2x2,
            PanelKind::PianoRoll => IconName::Piano,
            PanelKind::Playlist => IconName::ListMusic,
            PanelKind::Mixer => IconName::SlidersVertical,
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

    /// A fixed per-panel hue — not palette-switchable (like
    /// `theme::pattern_color`, these identify *which thing this is*, not
    /// *which theme is active*). Used for this panel's icon tint, its
    /// focus ring/glow, and its row-2 toggle pill, so four panels read as
    /// four distinct identities instead of one undifferentiated accent
    /// color repeated four times.
    pub fn identity_color(self) -> Hsla {
        match self {
            PanelKind::ChannelRack => Hsla { h: 255.0 / 360.0, s: 0.55, l: 0.70, a: 1.0 }, // indigo
            PanelKind::PianoRoll => Hsla { h: 175.0 / 360.0, s: 0.50, l: 0.62, a: 1.0 }, // teal
            PanelKind::Playlist => Hsla { h: 335.0 / 360.0, s: 0.60, l: 0.68, a: 1.0 }, // rose
            PanelKind::Mixer => Hsla { h: 145.0 / 360.0, s: 0.40, l: 0.62, a: 1.0 }, // sage
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

/// Allodium-tauri's own floating windows always carry a real two-layer
/// drop shadow (`box-shadow: 0 16px 48px rgba(8,10,20,0.45)` — a tight
/// contact shadow plus a wide ambient one), not the flat niri-style "only
/// while dragging" treatment this rewrite used before. Ported as two
/// `BoxShadow`s; `drag_boost` pushes both further out while the panel is
/// being moved, instead of swapping in an unrelated single shadow.
fn panel_shadow(drag_boost: bool) -> Vec<BoxShadow> {
    let lift = if drag_boost { 1.6 } else { 1.0 };
    vec![
        BoxShadow {
            color: theme::BG_APP.alpha(0.35),
            offset: point(px(0.0), px(8.0 * lift)),
            blur_radius: px(18.0 * lift),
            spread_radius: px(0.0),
            inset: false,
        },
        BoxShadow {
            color: theme::BG_APP.alpha(0.5),
            offset: point(px(0.0), px(22.0 * lift)),
            blur_radius: px(52.0 * lift),
            spread_radius: px(0.0),
            inset: false,
        },
    ]
}

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
        .bg(theme::BG_PANEL.alpha(0.94))
        .border_1()
        .border_color(theme::BORDER)
        .shadow(panel_shadow(dragging_this))
        .when(focused, |d| {
            let identity = kind.identity_color();
            d.border_color(identity).shadow(
                [panel_shadow(dragging_this), vec![BoxShadow {
                    color: identity.alpha(0.25),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(16.0),
                    spread_radius: px(0.0),
                    inset: false,
                }]]
                .concat(),
            )
        })
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
                .bg(theme::HEADER_BG)
                .border_b_1()
                .border_color(theme::BORDER)
                .cursor_default()
                .on_mouse_down(MouseButton::Left, on_drag_start)
                .child(Icon::new(kind.icon()).small().text_color(kind.identity_color()))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme::MAIN_TEXT)
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
                        .text_color(theme::accent_dim())
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::destructive()).text_color(theme::accent_text()))
                        .child(Icon::new(IconName::X).xsmall())
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
                .text_color(theme::accent_dim().alpha(0.35))
                .hover(|s| s.text_color(theme::accent_dim()))
                .child(Icon::new(IconName::ResizeCorner).xsmall())
                .on_mouse_down(MouseButton::Left, on_resize_start),
        )
}
