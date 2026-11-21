//! The toolbar row (Draw/Delete/Marquee tool buttons, the solo-play
//! button, the Note Properties toggle) and the channel-picker row below
//! it — Allodium-tauri's own `PianoRollHeader.svelte`, minus the window
//! chrome `floating_panel.rs` already provides generically for every
//! panel. See `piano_roll.rs`'s own top doc comment for how this came to
//! exist.

use gpui::assets::IconName;
use gpui::component::button::{Button, ButtonVariants};
use gpui::component::menu::{DropdownMenu, PopupMenuItem};
use gpui::{Context, IntoElement, ParentElement, Styled, div, px};

use crate::theme;
use crate::widgets::icon_toggle;

use super::PianoRoll;

/// The toolbar's tool selector — Allodium-tauri's own `EditTool` type
/// (`editTools.ts`), shared there with the Playlist; here it's just the
/// Piano Roll's own copy since the Playlist isn't in this rewrite's tool
/// system yet. Same semantics: Draw places/selects/moves/resizes notes,
/// Delete click-or-drags notes away, Marquee always drags a selection box.
/// The right mouse button deletes in every tool regardless of which of
/// these is active, and Ctrl+drag is always a marquee regardless of tool —
/// both handled directly in `interaction.rs`'s `handle_mouse_down`, not
/// through this enum.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum PianoRollTool {
    Draw,
    Delete,
    Marquee,
}

impl PianoRoll {
    /// A channel-picker dropdown — an easier way to switch which channel
    /// the Piano Roll shows than hunting down its row in the Channel Rack
    /// and double-clicking it. It's not a second notion of "which channel
    /// is open": picking an item here just calls the same
    /// `ChannelRack::select_channel` that double-click already does, so
    /// `sync_selection`'s existing live-follow keeps working unchanged —
    /// there is still only one "selected channel" in the whole app.
    pub(super) fn render_channel_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let rack = self.channel_rack.clone();
        let current = self.loaded_channel_id;
        let channels: Vec<(u32, String)> = self
            .channel_rack
            .read(cx)
            .channels()
            .iter()
            .filter(|c| !c.sample_path.is_empty())
            .map(|c| (c.id, c.name.clone()))
            .collect();
        let current_label =
            channels.iter().find(|(id, _)| Some(*id) == current).map(|(_, l)| l.clone()).unwrap_or_else(|| "No channel".to_string());

        div()
            .h(px(28.0))
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .bg(theme::HEADER_BG)
            .border_b_1()
            .border_color(theme::BORDER)
            .child(div().text_size(px(10.0)).text_color(theme::accent_dim()).child("Channel"))
            .child(
                Button::new("piano-roll-channel").ghost().compact().label(current_label).dropdown_menu(move |menu, _, _| {
                    channels.iter().cloned().fold(menu, |menu, (id, label)| {
                        let rack = rack.clone();
                        menu.item(PopupMenuItem::new(label).checked(Some(id) == current).on_click(move |_, _, cx| {
                            rack.update(cx, |r, cx| r.select_channel(id, cx));
                        }))
                    })
                }),
            )
    }

    /// Allodium-tauri's own `PianoRollHeader.svelte`: Draw/Delete/Marquee
    /// tool buttons, a Play toggle (solos and plays just the open channel —
    /// `toggle_solo_play`'s own doc comment), and the Note Properties
    /// toggle. The original bundles this with the window's own title/
    /// maximize/close chrome in one `.pr-header`; here that chrome already
    /// comes from `floating_panel.rs` generically for every panel, so this
    /// is just its left-hand controls as their own row.
    pub(super) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_playing = self.channel_rack.read(cx).is_playing();

        div()
            .h(px(28.0))
            .w_full()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .bg(theme::HEADER_BG)
            .border_b_1()
            .border_color(theme::BORDER)
            .child(icon_toggle(
                "pr-tool-draw",
                IconName::Pencil,
                self.tool == PianoRollTool::Draw,
                theme::accent(),
                26.0,
                26.0,
                cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.tool = PianoRollTool::Draw;
                    cx.notify();
                }),
            ))
            .child(icon_toggle(
                "pr-tool-delete",
                IconName::Eraser,
                self.tool == PianoRollTool::Delete,
                theme::accent(),
                26.0,
                26.0,
                cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.tool = PianoRollTool::Delete;
                    cx.notify();
                }),
            ))
            .child(icon_toggle(
                "pr-tool-marquee",
                IconName::SquareDashedMousePointer,
                self.tool == PianoRollTool::Marquee,
                theme::accent(),
                26.0,
                26.0,
                cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.tool = PianoRollTool::Marquee;
                    cx.notify();
                }),
            ))
            .child(div().w(px(1.0)).h(px(16.0)).bg(theme::BORDER))
            .child(icon_toggle(
                "pr-play",
                if is_playing { IconName::Pause } else { IconName::Play },
                is_playing,
                theme::accent(),
                26.0,
                26.0,
                cx.listener(|this, _: &gpui::ClickEvent, _, cx| this.toggle_solo_play(cx)),
            ))
            .child(icon_toggle(
                "pr-note-props",
                IconName::SlidersHorizontal,
                self.show_note_props,
                theme::accent(),
                26.0,
                26.0,
                cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.show_note_props = !this.show_note_props;
                    cx.notify();
                }),
            ))
    }

    /// The Piano Roll's own Play button: solos the open channel for the
    /// duration of playback (`ChannelRack::solo_only`/`restore_solo`)
    /// rather than toggling solo via the row's own mute dot — Allodium-
    /// tauri's `PianoRollHeader.svelte` setting `playback.soloChannelId`
    /// before starting and pressing it again just stops. A no-op with no
    /// channel open.
    fn toggle_solo_play(&mut self, cx: &mut Context<Self>) {
        let Some(channel_id) = self.loaded_channel_id else { return };
        let is_playing = self.channel_rack.read(cx).is_playing();
        self.channel_rack.update(cx, |rack, cx| {
            if is_playing {
                rack.toggle_play(cx);
                rack.restore_solo(cx);
            } else {
                rack.solo_only(channel_id, cx);
                rack.toggle_play(cx);
            }
        });
    }
}
