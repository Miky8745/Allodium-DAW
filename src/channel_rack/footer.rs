//! The rack's bottom bar: add-channel and the Pattern/Song transport
//! toggle. Allodium-tauri's own footer additionally has a horizontal
//! scrollbar for the growable step count — this rewrite's own lives just
//! above the footer instead, as its own thin strip (see
//! `channel_rack.rs`'s own `render` and `steps_scroll`).

use gpui::assets::IconName;
use gpui::component::{Icon, Sizable};
use gpui::{ClickEvent, Context, Div, ParentElement, Styled, div, prelude::*, px};

use crate::file_browser::SampleDragPayload;
use crate::theme;

use super::ChannelRack;

impl ChannelRack {
    pub(super) fn render_footer(&mut self, song_mode: bool, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .px_2()
            .h(px(26.0))
            .bg(theme::HEADER_BG)
            .border_t_1()
            .border_color(theme::BORDER)
            // Rounds its own bottom corners to match the floating panel's
            // outer rounded edge — GPUI's overflow clip is a plain
            // rectangle (see `floating_panel.rs`'s own note on this), so
            // without this, this square-cornered footer (the last thing
            // painted, flush against the panel's bottom edge) would show
            // past the panel's rounded corner instead of being clipped to
            // it.
            .rounded_b(theme::RADIUS_MD)
            .child(
                div()
                    .id("add-channel")
                    .w(px(20.0))
                    .h(px(20.0))
                    .rounded(theme::RADIUS_XS)
                    .border_1()
                    .border_color(theme::BTN_BORDER)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme::BTN_TEXT)
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::BTN_HOVER).text_color(theme::MAIN_TEXT))
                    .child(Icon::new(IconName::Plus).xsmall())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add_channel(cx)))
                    // Dropping a sample dragged from the File Browser here
                    // creates a new channel from it, same as the `rfd`
                    // file picker this button already opens on a click.
                    .drag_over::<SampleDragPayload>(|style, _, _, _| style.border_color(theme::accent()).bg(theme::accent().alpha(0.18)))
                    .on_drop(cx.listener(|this, payload: &SampleDragPayload, _, cx| {
                        this.add_channel_from_path(std::path::PathBuf::from(payload.path.clone()), cx);
                    })),
            )
            .child(
                div()
                    .id("transport-mode")
                    .h(px(20.0))
                    .px_2()
                    .rounded(theme::RADIUS_XS)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(10.0))
                    .bg(if song_mode { theme::accent() } else { theme::BG_PILL })
                    .cursor_pointer()
                    .hover(|s| if song_mode { s.bg(theme::accent_hover()) } else { s.bg(theme::BG_PILL_HOVER) })
                    .child(if song_mode { "Song" } else { "Pattern" })
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_song_mode(cx))),
            )
    }
}
