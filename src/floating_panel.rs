//! Generic floating-window chrome shared by the four floating panels
//! (Channel Rack, Piano Roll, Playlist, Mixer). None of those views know
//! they're floating — this module just wraps whatever they render in a
//! draggable/resizable/closable/focusable frame. See
//! `docs/stage-08-visual-redesign.md` for the design rationale (niri's
//! own flat, border-only-when-focused window convention, deliberately
//! not the original app's drop-shadow "SaaS card" look).
//!
//! The File Browser is **not** one of these — it's docked to the left
//! edge of the shell instead (see `shell.rs`), matching every ordinary
//! IDE's file tree rather than floating like a tool window. See
//! `file_browser.rs`'s own module doc for why.

use gpui::assets::IconName;
use gpui::component::{Icon, Sizable};
use gpui::{
    App, BoxShadow, ClickEvent, Context, Empty, Entity, Hsla, IntoElement, MouseButton,
    MouseDownEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, SharedString, Size,
    StatefulInteractiveElement, Styled, Window, div, point, prelude::*, px,
};

use crate::theme;

/// Carried by GPUI's own drag-and-drop machinery (`on_drag`/`on_drag_move`)
/// for every click-and-drag gesture this app does by hand: panel move,
/// panel resize, and the toolbar's BPM-pill drag (see `toolbar.rs`). Plain
/// `on_mouse_move` tracking on the workspace root broke once panels
/// started occluding each other: `InteractiveElement::occlude`'s
/// `BlockMouse` hit-test behavior stops hit-testing at the first occluded
/// hitbox it finds, which — since a div inserts its own hitbox before its
/// children's — also swallows every *ancestor* hitbox behind it, the
/// workspace root included. `on_drag_move` sidesteps this entirely: it's
/// dispatched by payload type during the capture phase, with no
/// hitbox/hover check at all, so it keeps firing regardless of what's
/// occluding what or where the pointer physically is.
#[derive(Clone, Copy)]
pub struct ShellDragMarker;

/// The view GPUI's drag API requires a constructor for. These drags
/// reposition real state (`PanelGeometry`, tempo) directly on every
/// `on_drag_move` tick rather than following a floating drag-preview
/// ghost, so this renders nothing.
pub struct DragPreview;

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

pub(crate) fn drag_preview(_: &ShellDragMarker, _: Point<Pixels>, _: &mut Window, cx: &mut App) -> Entity<DragPreview> {
    cx.new(|_| DragPreview)
}

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
    pub maximized: bool,
    /// The position/size to restore when un-maximizing — `Some` only
    /// while `maximized` is true. Allodium-tauri's own `FloatingWindow`
    /// leaves these untouched during the maximized stretch so restoring
    /// snaps back to exactly where the panel was, rather than recomputing
    /// a default layout.
    pub pre_maximize: Option<(Point<Pixels>, Size<Pixels>)>,
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
/// maximize/close buttons, draggable to move the panel) and a resize
/// handle at the bottom-right corner (hidden while maximized, matching
/// Allodium-tauri's own `FloatingWindow`). `on_focus` fires on a click
/// anywhere in the panel
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
    // Cancels a pending move/resize that never crossed GPUI's own drag
    // threshold (a plain click) — see `ShellDragMarker`'s doc comment and
    // `AppShell::cancel_pending_click`. Separate callbacks, one per
    // element, since each needs to be `is_hovered` on release for the
    // plain-click case, and a title-bar click and a resize-handle click
    // release over different elements.
    on_move_release: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
    on_resize_release: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
    on_maximize: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    // Allodium-tauri's own maximized state goes square (`border-radius:
    // 0`) rather than keeping the normal rounding against the workspace
    // edge it's now flush with. Piano Roll and Playlist stay square
    // unconditionally (not just while maximized): both scroll opaque
    // content (key rows, canvas-painted stripes/lanes) flush to their own
    // edges, and GPUI's `overflow_hidden` clips to a plain rectangle with
    // no rounded-corner variant, so any rounding on these two always let
    // scrolled content poke square past the curve. Simplest fix left
    // standing after chasing corner-masking tricks that didn't hold up
    // under scroll: don't round these two at all.
    let radius =
        if geometry.maximized || matches!(kind, PanelKind::PianoRoll | PanelKind::Playlist) { px(0.0) } else { theme::RADIUS_MD };
    div()
        .absolute()
        .left(geometry.position.x)
        .top(geometry.position.y)
        .w(geometry.size.width)
        .h(geometry.size.height)
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(radius)
        .bg(theme::BG_PANEL.alpha(0.94))
        .border_1()
        .border_color(theme::BORDER)
        .shadow(panel_shadow(dragging_this))
        // Stops clicks from reaching whatever panel sits behind this one
        // in z-order wherever they visually overlap — GPUI's hit-testing
        // otherwise still lets a lower hitbox handle the event even when
        // a higher one fully covers that pixel.
        .occlude()
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
                // Rounds this div's own top corners to match the outer
                // frame instead of depending on the frame's overflow clip
                // to do it — GPUI's content mask is always a plain
                // rectangle (its own `ContentMask` type carries only
                // `bounds`, never a shape), so a square-cornered child
                // flush against a rounded parent's edge visibly pokes out
                // past the rounded border rather than being clipped to it.
                .rounded_t(radius)
                .bg(theme::HEADER_BG)
                .border_b_1()
                .border_color(theme::BORDER)
                .cursor_default()
                .on_mouse_down(MouseButton::Left, on_drag_start)
                .on_mouse_up(MouseButton::Left, on_move_release)
                .on_drag(ShellDragMarker, drag_preview)
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
                        .id(("panel-maximize", kind as u8 as usize))
                        .w(px(18.0))
                        .h(px(18.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(theme::RADIUS_XS)
                        .text_color(theme::accent_dim())
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::BTN_HOVER).text_color(theme::MAIN_TEXT))
                        .child(Icon::new(if geometry.maximized { IconName::Minimize2 } else { IconName::Maximize }).xsmall())
                        .on_click(on_maximize),
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
        .child(div().flex_1().w_full().rounded_b(radius).overflow_hidden().child(body))
        .when(!geometry.maximized, |d| {
            d.child(
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
                    .on_mouse_down(MouseButton::Left, on_resize_start)
                    .on_mouse_up(MouseButton::Left, on_resize_release)
                    .on_drag(ShellDragMarker, drag_preview),
            )
        })
}
