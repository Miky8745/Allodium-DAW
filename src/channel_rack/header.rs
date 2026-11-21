//! The rack's own control bar: left-aligned transport/utility controls, a
//! spacer where Allodium-tauri's original window chrome put its title
//! (this rewrite's panel chrome — `floating_panel.rs` — already shows
//! "Channel Rack" one level up, so this row doesn't repeat it), and
//! right-aligned time/feel controls. See `channel_rack.rs`'s own top doc
//! comment for which of these are wired to real behavior vs. carried
//! over as the same cosmetic no-ops the original has.

use gpui::assets::IconName;
use gpui::component::button::{Button, ButtonVariants};
use gpui::component::menu::{DropdownMenu, PopupMenuItem};
use gpui::component::{Icon, Sizable};
use gpui::{App, ClickEvent, Context, Div, MouseButton, MouseDownEvent, ParentElement, ScrollEvent, Styled, Window, div, prelude::*, px};

use crate::theme;
use crate::widgets::{dial, group_label, icon_toggle, scroll_field};

use super::{ChannelRack, ChannelRackEvent, RackFilter};

impl ChannelRack {
    /// The selected pattern's length, in whole bars (its own `length_beats`
    /// divided by `beats_per_bar`) — what the "LEN" scroll field shows and
    /// steps, matching the original's `patternLength`/`barLength` pairing
    /// exactly (a *step* of one bar, not one beat).
    fn selected_pattern_length_bars(&self) -> u32 {
        let bar = (self.project.beats_per_bar as f64).max(1.0);
        self.project
            .patterns
            .items
            .iter()
            .find(|p| p.id == self.project.selected_pattern_id)
            .map(|p| (p.length_beats / bar).round().max(1.0) as u32)
            .unwrap_or(1)
    }

    fn adjust_selected_pattern_length(&mut self, delta_bars: i32, cx: &mut Context<Self>) {
        let bar = (self.project.beats_per_bar as f64).max(1.0);
        let current_bars = self.selected_pattern_length_bars() as i32;
        let new_bars = (current_bars + delta_bars).max(1) as f64;
        let pattern_id = self.project.selected_pattern_id;
        if let Some(p) = self.project.patterns.items.iter_mut().find(|p| p.id == pattern_id) {
            p.length_beats = new_bars * bar;
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    pub(super) fn render_header(&mut self, cx: &mut Context<Self>) -> Div {
        let playing = self.engine.is_playing();
        let rack = cx.entity();

        div()
            .h(px(32.0))
            .w_full()
            .flex()
            .flex_shrink_0()
            .items_center()
            .px_2()
            .gap_2()
            .bg(theme::HEADER_BG)
            .border_b_1()
            .border_color(theme::BORDER)
            .cursor_default()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("rack-options")
                            .ghost()
                            .compact()
                            .icon(IconName::Settings2)
                            .dropdown_caret(false)
                            .dropdown_menu(|menu, _, _| {
                                menu.item(PopupMenuItem::new("Channel settings").on_click(|_, _, _| {}))
                                    .item(PopupMenuItem::new("Rename").on_click(|_, _, _| {}))
                                    .item(PopupMenuItem::new("Color").on_click(|_, _, _| {}))
                            }),
                    )
                    .child(icon_toggle(
                        "loop-start",
                        IconName::RotateCcw,
                        self.loop_start,
                        theme::accent(),
                        26.0,
                        26.0,
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.loop_start = !this.loop_start;
                            cx.notify();
                        }),
                    ))
                    .child({
                        let filter = self.filter;
                        let rack = rack.clone();
                        Button::new("rack-filter")
                            .ghost()
                            .compact()
                            .icon(IconName::Funnel)
                            .label(if filter == RackFilter::All { "ALL" } else { "UNSORTED" })
                            .dropdown_menu(move |menu, _, cx| {
                                let filter = rack.read(cx).filter;
                                let rack_all = rack.clone();
                                let rack_unsorted = rack.clone();
                                menu.item(
                                    PopupMenuItem::new("All").checked(filter == RackFilter::All).on_click(move |_, _, cx| {
                                        rack_all.update(cx, |r, cx| {
                                            r.filter = RackFilter::All;
                                            cx.notify();
                                        });
                                    }),
                                )
                                .item(
                                    PopupMenuItem::new("Unsorted").checked(filter == RackFilter::Unsorted).on_click(move |_, _, cx| {
                                        rack_unsorted.update(cx, |r, cx| {
                                            r.filter = RackFilter::Unsorted;
                                            cx.notify();
                                        });
                                    }),
                                )
                            })
                    })
                    .child(
                        div()
                            .id("name-col-resize")
                            .w(px(16.0))
                            .h(px(26.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(theme::RADIUS_XS)
                            .cursor_col_resize()
                            .text_color(theme::BTN_TEXT.alpha(0.5))
                            .hover(|s| s.bg(theme::BTN_HOVER).text_color(theme::MAIN_TEXT))
                            .child(Icon::new(IconName::GripVertical).xsmall())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, ev: &MouseDownEvent, _, _| this.begin_col_resize(f32::from(ev.position.x))),
                            ),
                    )
                    .child(icon_toggle(
                        "rack-play",
                        if playing { IconName::Square } else { IconName::Play },
                        playing,
                        theme::accent(),
                        26.0,
                        26.0,
                        cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_play(cx)),
                    )),
            )
            // No centered title here — the floating-panel chrome one level
            // up (`floating_panel.rs`) already shows "Channel Rack"; a
            // second copy in this content-level header would just repeat
            // it right below the first.
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_0p5()
                            .child(dial(
                                "swing",
                                "Swing",
                                format!("{:.0}%", self.swing * 100.0),
                                self.swing,
                                1.0,
                                210.0,
                                300.0,
                                26.0,
                                cx.listener(|this, ev: &MouseDownEvent, _, _| this.begin_swing_drag(f32::from(ev.position.y))),
                                {
                                    let rack = rack.clone();
                                    move |_: &mut Window, cx: &mut App| {
                                        rack.update(cx, |r, cx| {
                                            r.swing = 0.0;
                                            cx.notify();
                                        })
                                    }
                                },
                                cx.listener(|_this, (hovering, name, value): &(bool, &'static str, String), _, cx| {
                                    if *hovering {
                                        cx.emit(ChannelRackEvent::HoverInfo(name, value.clone()));
                                    }
                                }),
                            ))
                            .child(group_label("SWING")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_0p5()
                            .child(scroll_field(
                                "time-sig",
                                format!("{}/4", self.beats_per_bar()),
                                28.0,
                                cx.listener(|this, ev: &ScrollEvent, _, cx| {
                                    let dy = f32::from(ev.delta.pixel_delta(px(1.0)).y);
                                    let sign: i32 = if dy > 0.0 { -1 } else if dy < 0.0 { 1 } else { 0 };
                                    let next = (this.beats_per_bar() as i32 + sign).max(1) as u32;
                                    this.set_beats_per_bar(next, cx);
                                }),
                            ))
                            .child(group_label("TIME SIG")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_0p5()
                            .child(scroll_field(
                                "pattern-length",
                                format!("{} bar{}", self.selected_pattern_length_bars(), if self.selected_pattern_length_bars() == 1 { "" } else { "s" }),
                                48.0,
                                cx.listener(|this, ev: &ScrollEvent, _, cx| {
                                    let dy = f32::from(ev.delta.pixel_delta(px(1.0)).y);
                                    let sign: i32 = if dy > 0.0 { -1 } else if dy < 0.0 { 1 } else { 0 };
                                    this.adjust_selected_pattern_length(sign, cx);
                                }),
                            ))
                            .child(group_label("LEN")),
                    )
                    .child(icon_toggle(
                        "loop-mode",
                        IconName::Repeat,
                        self.loop_mode,
                        theme::accent(),
                        26.0,
                        26.0,
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.loop_mode = !this.loop_mode;
                            cx.notify();
                        }),
                    ))
                    .child(icon_toggle(
                        "graph-editor",
                        IconName::ChartNoAxesColumn,
                        self.graph_editor,
                        theme::accent(),
                        26.0,
                        26.0,
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.graph_editor = !this.graph_editor;
                            cx.notify();
                        }),
                    ))
                    .child(icon_toggle(
                        "piano-overview",
                        IconName::LayoutGrid,
                        self.piano_roll_overview,
                        theme::accent(),
                        26.0,
                        26.0,
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.piano_roll_overview = !this.piano_roll_overview;
                            cx.notify();
                        }),
                    )),
            )
    }
}
