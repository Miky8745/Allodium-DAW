//! The app shell: one toolbar row + a floating-window workspace.
//! Channel Rack, Piano Roll, Playlist, and Mixer are draggable/
//! resizable/closable panels (see `src/floating_panel.rs`), reopened via
//! the toolbar's toggle pills — the original app's "window-based" model,
//! redesigned fresh rather than ported. See
//! `docs/stage-08-visual-redesign.md`.

use std::sync::Arc;

use gpui::{
    AppContext, ClickEvent, Context, Entity, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, Styled, Window, div, point,
    prelude::*, px, size,
};

use crate::audio::AudioEngine;
use crate::channel_rack::ChannelRack;
use crate::floating_panel::{self, PanelDrag, PanelDragKind, PanelGeometry, PanelKind, MIN_PANEL_SIZE};
use crate::mixer::Mixer;
use crate::perf::FrameStats;
use crate::piano_roll::PianoRoll;
use crate::playlist::Playlist;
use crate::theme;

struct PanelGeometries {
    channel_rack: PanelGeometry,
    piano_roll: PanelGeometry,
    playlist: PanelGeometry,
    mixer: PanelGeometry,
}

impl PanelGeometries {
    fn get(&self, kind: PanelKind) -> PanelGeometry {
        match kind {
            PanelKind::ChannelRack => self.channel_rack,
            PanelKind::PianoRoll => self.piano_roll,
            PanelKind::Playlist => self.playlist,
            PanelKind::Mixer => self.mixer,
        }
    }

    fn get_mut(&mut self, kind: PanelKind) -> &mut PanelGeometry {
        match kind {
            PanelKind::ChannelRack => &mut self.channel_rack,
            PanelKind::PianoRoll => &mut self.piano_roll,
            PanelKind::Playlist => &mut self.playlist,
            PanelKind::Mixer => &mut self.mixer,
        }
    }
}

/// Initial layout roughly mirrors the previous fixed stacked order
/// (Channel Rack, then Piano Roll, then Playlist down the left; Mixer to
/// the right) — familiar on first launch, freely rearrangeable after.
impl Default for PanelGeometries {
    fn default() -> Self {
        let geo = |x: f32, y: f32, w: f32, h: f32| PanelGeometry {
            position: point(px(x), px(y)),
            size: size(px(w), px(h)),
            open: true,
        };
        Self {
            channel_rack: geo(16.0, 16.0, 860.0, 260.0),
            piano_roll: geo(16.0, 292.0, 860.0, 270.0),
            playlist: geo(16.0, 578.0, 860.0, 220.0),
            mixer: geo(892.0, 16.0, 380.0, 500.0),
        }
    }
}

pub struct AppShell {
    frame_stats: FrameStats,
    channel_rack: Entity<ChannelRack>,
    piano_roll: Entity<PianoRoll>,
    playlist: Entity<Playlist>,
    mixer: Entity<Mixer>,
    geometry: PanelGeometries,
    z_order: Vec<PanelKind>,
    drag: Option<PanelDrag>,
}

impl AppShell {
    pub fn new(engine: Arc<AudioEngine>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let channel_rack = cx.new(|cx| ChannelRack::new(engine, window, cx));
        let piano_roll = cx.new(|cx| PianoRoll::new(channel_rack.clone(), window, cx));
        let playlist = cx.new(|cx| Playlist::new(channel_rack.clone(), window, cx));
        let mixer = cx.new(|cx| Mixer::new(channel_rack.clone(), window, cx));
        Self {
            frame_stats: FrameStats::new(),
            channel_rack,
            piano_roll,
            playlist,
            mixer,
            geometry: PanelGeometries::default(),
            z_order: PanelKind::ALL.to_vec(),
            drag: None,
        }
    }

    fn is_frontmost(&self, kind: PanelKind) -> bool {
        self.z_order.last() == Some(&kind)
    }

    fn focus_panel(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        self.z_order.retain(|k| *k != kind);
        self.z_order.push(kind);
        cx.notify();
    }

    /// Closed → open + focus; open-but-not-frontmost → bring to front
    /// (stays open); open-and-already-frontmost → close. The original's
    /// exact 3-state toolbar semantics.
    fn toggle_panel(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        let open = self.geometry.get(kind).open;
        if !open {
            self.geometry.get_mut(kind).open = true;
            self.focus_panel(kind, cx);
        } else if !self.is_frontmost(kind) {
            self.focus_panel(kind, cx);
        } else {
            self.geometry.get_mut(kind).open = false;
            cx.notify();
        }
    }

    fn close_panel(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        self.geometry.get_mut(kind).open = false;
        cx.notify();
    }

    fn begin_drag(&mut self, kind: PanelKind, drag_kind: PanelDragKind, mouse: Point<Pixels>, cx: &mut Context<Self>) {
        let geo = self.geometry.get(kind);
        self.drag = Some(PanelDrag { kind: drag_kind, panel: kind, start_mouse: mouse, start_position: geo.position, start_size: geo.size });
        self.focus_panel(kind, cx);
    }

    fn update_drag(&mut self, mouse: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.drag else { return };
        let dx = mouse.x - drag.start_mouse.x;
        let dy = mouse.y - drag.start_mouse.y;
        let geo = self.geometry.get_mut(drag.panel);
        match drag.kind {
            PanelDragKind::Move => {
                geo.position.x = (drag.start_position.x + dx).max(px(0.0));
                geo.position.y = (drag.start_position.y + dy).max(px(0.0));
            }
            PanelDragKind::Resize => {
                geo.size.width = (drag.start_size.width + dx).max(MIN_PANEL_SIZE.width);
                geo.size.height = (drag.start_size.height + dy).max(MIN_PANEL_SIZE.height);
            }
        }
        cx.notify();
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            cx.notify();
        }
    }

    fn toggle_pill(&self, kind: PanelKind, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.geometry.get(kind).open;
        let frontmost = open && self.is_frontmost(kind);
        let mut pill = div()
            .id(("panel-toggle", kind as u8 as usize))
            .h(px(26.0))
            .px_2()
            .rounded(theme::RADIUS_SM)
            .flex()
            .items_center()
            .gap_1()
            .text_size(px(11.0))
            .cursor_pointer()
            .child(div().child(kind.glyph()))
            .child(kind.label())
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_panel(kind, cx)));
        pill = if frontmost {
            pill.bg(theme::ACCENT).text_color(theme::ACCENT_TEXT)
        } else if open {
            pill.bg(theme::BG_PILL).text_color(theme::ACCENT).border_1().border_color(theme::ACCENT_DIM)
        } else {
            pill.bg(theme::BG_PILL).text_color(theme::ACCENT_DIM)
        };
        pill
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.frame_stats.tick();
        window.request_animation_frame();
        cx.notify();

        let piano_roll_channel_name = {
            let rack = self.channel_rack.read(cx);
            rack.selected_channel_id().and_then(|id| rack.channels().iter().find(|c| c.id == id)).map(|c| {
                if c.name.is_empty() { "Drop sample".to_string() } else { c.name.clone() }
            })
        };
        let piano_roll_title = match &piano_roll_channel_name {
            Some(name) => format!("Piano Roll — {name}"),
            None => "Piano Roll".to_string(),
        };

        let available_mixer_h = self.geometry.get(PanelKind::Mixer).size.height - px(26.0);
        self.mixer.update(cx, |m, cx| m.set_fader_height((available_mixer_h - px(140.0)).max(px(40.0)), cx));

        let mut workspace = div().absolute().left_0().top_0().size_full();
        for kind in self.z_order.clone() {
            let geo = self.geometry.get(kind);
            if !geo.open {
                continue;
            }
            let focused = self.is_frontmost(kind);
            let dragging_this = self.drag.map(|d| d.panel == kind).unwrap_or(false);
            let title = match kind {
                PanelKind::PianoRoll => piano_roll_title.clone(),
                other => other.label().to_string(),
            };
            let body: gpui::AnyElement = match kind {
                PanelKind::ChannelRack => self.channel_rack.clone().into_any_element(),
                PanelKind::PianoRoll => self.piano_roll.clone().into_any_element(),
                PanelKind::Playlist => self.playlist.clone().into_any_element(),
                PanelKind::Mixer => self.mixer.clone().into_any_element(),
            };
            workspace = workspace.child(floating_panel::render_panel(
                kind,
                title,
                geo,
                focused,
                dragging_this,
                body,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| this.focus_panel(kind, cx)),
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| this.begin_drag(kind, PanelDragKind::Move, ev.position, cx)),
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| this.begin_drag(kind, PanelDragKind::Resize, ev.position, cx)),
                cx.listener(move |this, _: &ClickEvent, _, cx| this.close_panel(kind, cx)),
            ));
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::BG_APP)
            .font_family(theme::FONT_UI)
            .child(
                div()
                    .h(theme::TOOLBAR_H)
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .bg(theme::BG_PANEL)
                    .border_b_1()
                    .border_color(theme::BORDER)
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme::ACCENT)
                            .child("Allodium"),
                    )
                    .child({
                        let mut pills = div().flex().gap_1();
                        for kind in PanelKind::ALL {
                            pills = pills.child(self.toggle_pill(kind, cx));
                        }
                        pills
                    }),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .bg(theme::BG_APP)
                    .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                        if this.drag.is_some() {
                            this.update_drag(ev.position, cx);
                        }
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_drag(cx)))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_drag(cx)))
                    .child(workspace),
            )
    }
}
