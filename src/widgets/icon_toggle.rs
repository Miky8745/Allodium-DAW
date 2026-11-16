use gpui::assets::IconName;
use gpui::component::{Icon, Sizable};
use gpui::{App, ClickEvent, Div, Hsla, ParentElement, Stateful, Styled, Window, div, prelude::*, px};

use crate::theme;

/// A small square icon button with an on/off state, tinted `color` when
/// active — the shape behind every plain icon toggle in this app (Channel
/// Rack's header row; `toolbar::toggle` layers its own glow/hover-info on
/// top of this for the main toolbar's row). Not every icon button in the
/// app goes through this: a *bordered* one (`toolbar::transport_btn`) is
/// a different enough look to stay its own thing rather than be forced
/// through one do-everything component.
#[allow(clippy::too_many_arguments)]
pub fn icon_toggle(
    id: impl Into<gpui::ElementId>,
    icon: IconName,
    active: bool,
    color: Hsla,
    width: f32,
    height: f32,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(width))
        .h(px(height))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::RADIUS_XS)
        .cursor_pointer()
        .text_color(if active { color } else { theme::BTN_TEXT })
        .when(active, |d| d.bg(color.alpha(0.1)))
        .hover(|s| if active { s } else { s.bg(theme::BTN_HOVER).text_color(theme::MAIN_TEXT) })
        .child(Icon::new(icon).small())
        .on_click(on_click)
}
