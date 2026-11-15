//! The Preferences dialog — General / Theme / MIDI tabs, opened from the
//! toolbar's Edit menu (see `src/toolbar.rs`'s dropdown on "Edit"). It's an
//! in-app dialog (`gpui::component::dialog::Dialog`), not a second OS
//! window.
//!
//! Theme is the one tab with something real behind it: picking a palette
//! calls `theme::apply_palette` immediately, and the whole app repaints on
//! its next frame — `AppShell` already redraws continuously, so there's no
//! extra invalidation to wire up. General and MIDI have no backend to
//! route to yet (no settings persistence anywhere in this codebase, no MIDI
//! I/O at all — see the toolbar's own inert MIDI LED), so they're honest
//! placeholders: shaped like the eventual settings, each row saying
//! plainly what's not implemented rather than presenting a control that
//! looks live but silently does nothing.

use gpui::assets::IconName;
use gpui::component::form::{field, v_form};
use gpui::component::tab::{Tab, TabBar};
use gpui::component::{ActiveTheme, Icon, Sizable};
use gpui::{App, Entity, IntoElement, ParentElement, Styled, div, prelude::*, px};

use crate::theme::{self, Palette};

#[derive(Clone, Copy, PartialEq, Eq)]
enum PrefsTab {
    General,
    Theme,
    Midi,
}

pub struct Preferences {
    tab: PrefsTab,
}

impl Preferences {
    pub fn new() -> Self {
        Self { tab: PrefsTab::General }
    }
}

/// Builds the dialog body. Takes the persistent `Entity<Preferences>`
/// rather than owning the state itself — `window.open_dialog`'s builder
/// may run again on every rebuild while the dialog is open, so the tab
/// selection has to live outside it to survive that.
pub fn render(prefs: &Entity<Preferences>, cx: &mut App) -> impl IntoElement {
    let tab = prefs.read(cx).tab;
    div()
        .flex()
        .flex_col()
        .gap_3()
        .w(px(480.0))
        .child(render_tab_bar(prefs, tab))
        .child(match tab {
            PrefsTab::General => render_general(cx).into_any_element(),
            PrefsTab::Theme => render_theme(cx).into_any_element(),
            PrefsTab::Midi => render_midi(cx).into_any_element(),
        })
}

fn render_tab_bar(prefs: &Entity<Preferences>, tab: PrefsTab) -> impl IntoElement {
    let prefs = prefs.clone();
    TabBar::new("prefs-tabs")
        .segmented()
        .children([Tab::new().label("General"), Tab::new().label("Theme"), Tab::new().label("MIDI")])
        .selected_index(match tab {
            PrefsTab::General => 0,
            PrefsTab::Theme => 1,
            PrefsTab::Midi => 2,
        })
        .on_click(move |ix: &usize, _window, cx| {
            let next = match ix {
                0 => PrefsTab::General,
                1 => PrefsTab::Theme,
                _ => PrefsTab::Midi,
            };
            prefs.update(cx, |p, cx| {
                p.tab = next;
                cx.notify();
            });
        })
}

fn placeholder_value(cx: &App, text: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .rounded(theme::RADIUS_SM)
        .bg(theme::BG_PILL)
        .text_size(px(11.0))
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

fn render_general(cx: &App) -> impl IntoElement {
    v_form()
        .child(
            field()
                .label("Audio output device")
                .description("No device picker yet — the engine opens the system default output.")
                .child(placeholder_value(cx, "System default")),
        )
        .child(field().label("Sample rate").description("Not configurable yet.").child(placeholder_value(cx, "48,000 Hz")))
        .child(field().label("Buffer size").description("Not configurable yet.").child(placeholder_value(cx, "512 samples")))
}

fn render_midi(cx: &App) -> impl IntoElement {
    v_form()
        .child(
            field()
                .label("MIDI input device")
                .description("No MIDI I/O anywhere in this rewrite yet.")
                .child(placeholder_value(cx, "None")),
        )
        .child(
            field()
                .label("MIDI output device")
                .description("No MIDI I/O anywhere in this rewrite yet.")
                .child(placeholder_value(cx, "None")),
        )
}

fn render_theme(cx: &mut App) -> impl IntoElement {
    let active = theme::active_palette();
    div()
        .flex()
        .flex_wrap()
        .gap_2()
        .children(theme::PALETTES.iter().enumerate().map(|(ix, p)| render_swatch(cx, ix, *p, p.id == active.id)))
}

fn render_swatch(cx: &App, ix: usize, palette: Palette, selected: bool) -> impl IntoElement {
    div()
        .id(("theme-swatch", ix))
        .w(px(146.0))
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded(theme::RADIUS_MD)
        .cursor_pointer()
        .border_1()
        .border_color(if selected { palette.accent } else { theme::BORDER })
        .bg(if selected { palette.accent.alpha(0.12) } else { theme::BG_PILL })
        .hover(|s| s.bg(palette.accent.alpha(0.18)))
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().size(px(16.0)).rounded_full().bg(palette.accent))
                .child(div().size(px(16.0)).rounded_full().bg(palette.secondary))
                .child(div().size(px(16.0)).rounded_full().bg(palette.destructive))
                .when(selected, |d| d.child(div().flex_1()).child(Icon::new(IconName::CircleDot).small().text_color(palette.accent))),
        )
        .child(div().text_size(px(12.0)).font_weight(gpui::FontWeight::BOLD).child(palette.name))
        .child(div().text_size(px(9.5)).text_color(cx.theme().muted_foreground).child(palette.description))
        .on_click(move |_, _, cx| theme::apply_palette(cx, palette))
}
