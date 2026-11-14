//! The three-zone app shell: toolbar (two rows) + left panel + workspace,
//! mirroring Allodium-tauri's own layout convention (see its `CLAUDE.md`
//! "Views" section). Empty chrome only for Stage 1 — each zone's real
//! content arrives with its own later stage.

use std::sync::Arc;

use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};

use crate::audio::AudioEngine;
use crate::channel_rack::ChannelRack;
use crate::perf::FrameStats;
use crate::theme;

pub struct AppShell {
    frame_stats: FrameStats,
    channel_rack: Entity<ChannelRack>,
}

impl AppShell {
    pub fn new(engine: Arc<AudioEngine>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let channel_rack = cx.new(|cx| ChannelRack::new(engine, window, cx));
        Self {
            frame_stats: FrameStats::new(),
            channel_rack,
        }
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.frame_stats.tick();
        window.request_animation_frame();
        cx.notify();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::BG_APP)
            .font_family(theme::FONT_UI)
            .child(
                // Toolbar row 1.
                div()
                    .h(theme::TOOLBAR_ROW1_H)
                    .w_full()
                    .bg(theme::BG_PANEL)
                    .border_b_1()
                    .border_color(theme::BORDER),
            )
            .child(
                // Toolbar row 2.
                div()
                    .h(theme::TOOLBAR_ROW2_H)
                    .w_full()
                    .bg(theme::BG_PANEL)
                    .border_b_1()
                    .border_color(theme::BORDER),
            )
            .child(
                // Left panel + workspace.
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .flex_row()
                    .child(
                        div()
                            .w(theme::LEFT_PANEL_W)
                            .h_full()
                            .bg(theme::BG_PANEL)
                            .border_r_1()
                            .border_color(theme::BORDER),
                    )
                    .child(div().flex_1().h_full().bg(theme::BG_APP).child(self.channel_rack.clone())),
            )
    }
}
