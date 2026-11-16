//! Mixer: a row of vertical strips (Master + one per `MixerTrack`), each
//! with a volume fader, pan, mute, solo. Real audio routing, wired into
//! the audio engine's mixdown stage in `src/audio/mod.rs` — see
//! `docs/stage-07-mixer.md`.
//!
//! No insert-effect rack, no multi-target routing, no strip renaming, per
//! `../daw-rewrite-starter/stages/07-mixer.md`'s explicit scope cut.

use gpui::{
    ClickEvent, Context, Entity, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Render, Styled, Window, div, prelude::*, px,
};

use crate::channel_rack::ChannelRack;
use crate::theme;

const STRIP_W: f32 = 76.0;
const DEFAULT_FADER_H: f32 = 110.0;
const FADER_W: f32 = 18.0;

#[derive(Clone, Copy)]
enum FaderKind {
    MasterVolume,
    TrackVolume(u32),
    TrackPan(u32),
}

struct FaderDrag {
    kind: FaderKind,
    start_y: f32,
    start_value: f64,
}

pub struct Mixer {
    channel_rack: Entity<ChannelRack>,
    drag: Option<FaderDrag>,
    /// The panel's available height minus chrome, computed by `AppShell`
    /// from the floating panel's actual (resizable) size each frame —
    /// no DOM-measurement trick needed, since the parent already owns
    /// the panel's exact size as plain Rust state.
    fader_height: gpui::Pixels,
}

impl Mixer {
    pub fn new(channel_rack: Entity<ChannelRack>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { channel_rack, drag: None, fader_height: px(DEFAULT_FADER_H) }
    }

    pub fn set_fader_height(&mut self, height: gpui::Pixels, cx: &mut Context<Self>) {
        if self.fader_height != height {
            self.fader_height = height;
            cx.notify();
        }
    }

    fn begin_drag(&mut self, kind: FaderKind, start_y: f32, cx: &Context<Self>) {
        let rack = self.channel_rack.read(cx);
        let start_value = match kind {
            FaderKind::MasterVolume => rack.master_volume(),
            FaderKind::TrackVolume(id) => rack.mixer_tracks().iter().find(|t| t.id == id).map(|t| t.volume).unwrap_or(0.8),
            FaderKind::TrackPan(id) => rack.mixer_tracks().iter().find(|t| t.id == id).map(|t| t.pan).unwrap_or(0.5),
        };
        self.drag = Some(FaderDrag { kind, start_y, start_value });
    }

    fn update_drag(&mut self, current_y: f32, cx: &mut Context<Self>) {
        let Some(drag) = &self.drag else { return };
        // Dragging up increases the value, so the delta is inverted.
        let height = match drag.kind {
            FaderKind::TrackPan(_) => 1.0,
            _ => f32::from(self.fader_height),
        };
        let delta = (drag.start_y - current_y) / height;
        let new_value = (drag.start_value + delta as f64).clamp(0.0, 1.0);
        match drag.kind {
            FaderKind::MasterVolume => self.channel_rack.update(cx, |rack, cx| rack.set_master_volume(new_value, cx)),
            FaderKind::TrackVolume(id) => {
                self.channel_rack.update(cx, |rack, cx| rack.set_mixer_track_volume(id, new_value, cx))
            }
            FaderKind::TrackPan(id) => self.channel_rack.update(cx, |rack, cx| rack.set_mixer_track_pan(id, new_value, cx)),
        }
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            self.channel_rack.update(cx, |rack, _| rack.persist());
        }
    }
}

/// A vertical fader: a thin track with a fill proportional to `value`,
/// dragged like the Channel Rack's horizontal sliders (relative delta
/// from mouse-down, not an absolute position — GPUI's plain `div()`
/// doesn't hand event handlers its own screen bounds, same reasoning as
/// `channel_rack.rs`'s `slider`).
fn fader(value: f64, kind: FaderKind, height: gpui::Pixels, cx: &Context<Mixer>) -> impl IntoElement {
    let fill_h = (value.clamp(0.0, 1.0) as f32) * f32::from(height);
    div()
        .h(height)
        .w(px(FADER_W))
        .rounded(px(4.0))
        .bg(theme::BG_PANEL)
        .relative()
        .child(div().absolute().left_0().bottom_0().w_full().h(px(fill_h)).rounded(px(4.0)).bg(theme::accent()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| this.begin_drag(kind, f32::from(ev.position.y), cx)),
        )
}

fn pan_dot(pan: f64, track_id: u32, cx: &Context<Mixer>) -> impl IntoElement {
    let left = (pan.clamp(0.0, 1.0) as f32) * (STRIP_W - 32.0);
    div()
        .w(px(STRIP_W - 16.0))
        .h(px(14.0))
        .rounded(px(7.0))
        .bg(theme::BG_PANEL)
        .relative()
        .child(div().absolute().left(px(left)).top(px(2.0)).w(px(10.0)).h(px(10.0)).rounded(px(5.0)).bg(theme::accent_dim()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                this.begin_drag(FaderKind::TrackPan(track_id), f32::from(ev.position.x), cx)
            }),
        )
}

impl Render for Mixer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rack = self.channel_rack.read(cx);
        let master_volume = rack.master_volume();
        let tracks: Vec<_> = rack.mixer_tracks().to_vec();

        let mut strips = div().flex().flex_row().gap_2().p_2();

        strips = strips.child(
            div()
                .w(px(STRIP_W))
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .p_2()
                .rounded(theme::RADIUS_SM)
                .bg(theme::BG_PILL)
                .child(div().text_size(px(11.0)).text_color(theme::accent()).child("Master"))
                .child(fader(master_volume, FaderKind::MasterVolume, self.fader_height, cx)),
        );

        for t in &tracks {
            let id = t.id;
            strips = strips.child(
                div()
                    .w(px(STRIP_W))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .p_2()
                    .rounded(theme::RADIUS_SM)
                    .bg(theme::BG_PILL)
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_size(px(11.0)).text_color(theme::accent_dim()).child(format!("Trk {id}")))
                            .child(
                                div()
                                    .id(("remove-track", id as usize))
                                    .rounded(theme::RADIUS_XS)
                                    .text_color(theme::destructive())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme::destructive().alpha(0.15)))
                                    .child("×")
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.channel_rack.update(cx, |rack, cx| rack.remove_mixer_track(id, cx));
                                    })),
                            ),
                    )
                    .child(pan_dot(t.pan, id, cx))
                    .child(fader(t.volume, FaderKind::TrackVolume(id), self.fader_height, cx))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(
                                div()
                                    .id(("mix-mute", id as usize))
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.0))
                                    .bg(if t.muted { theme::destructive() } else { theme::BG_PANEL })
                                    .cursor_pointer()
                                    .hover(|s| if t.muted { s } else { s.bg(theme::BG_PILL_HOVER) })
                                    .child("M")
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.channel_rack.update(cx, |rack, cx| rack.toggle_mixer_mute(id, cx));
                                    })),
                            )
                            .child(
                                div()
                                    .id(("mix-solo", id as usize))
                                    .w(px(24.0))
                                    .h(px(18.0))
                                    .rounded(theme::RADIUS_XS)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.0))
                                    .bg(if t.soloed { theme::accent() } else { theme::BG_PANEL })
                                    .cursor_pointer()
                                    .hover(|s| if t.soloed { s } else { s.bg(theme::BG_PILL_HOVER) })
                                    .child("S")
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.channel_rack.update(cx, |rack, cx| rack.toggle_mixer_solo(id, cx));
                                    })),
                            ),
                    ),
            );
        }

        strips = strips.child(
            div()
                .id("add-mixer-track")
                .w(px(STRIP_W))
                .h(px(32.0))
                .rounded(theme::RADIUS_SM)
                .flex()
                .items_center()
                .justify_center()
                .bg(theme::BG_PILL)
                .text_size(px(11.0))
                .cursor_pointer()
                .hover(|s| s.bg(theme::BG_PILL_HOVER))
                .active(|s| s.bg(theme::BG_PILL_ACTIVE))
                .child("+ Track")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.channel_rack.update(cx, |rack, cx| rack.add_mixer_track(cx));
                })),
        );

        div()
            .size_full()
            .overflow_hidden()
            .bg(theme::BG_APP)
            // Rounds its own bottom corners to match the floating panel's
            // outer rounded edge — GPUI's overflow clip is a plain
            // rectangle, so without this, this square-cornered full-size
            // background would show past the panel's rounded corner
            // instead of being clipped to it (see `floating_panel.rs`'s
            // own note on the same limitation).
            .rounded_b(theme::RADIUS_MD)
            .font_family(theme::FONT_UI)
            .text_color(theme::accent_dim())
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if this.drag.is_some() {
                    let y = f32::from(ev.position.y);
                    let x = f32::from(ev.position.x);
                    let use_x = matches!(this.drag.as_ref().map(|d| d.kind), Some(FaderKind::TrackPan(_)));
                    this.update_drag(if use_x { x } else { y }, cx);
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_drag(cx)))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_drag(cx)))
            .child(strips)
    }
}
