//! The three-zone app shell: toolbar (two rows) + left panel + workspace,
//! mirroring Allodium-tauri's own layout convention (see its `CLAUDE.md`
//! "Views" section). Empty chrome only for Stage 1 — each zone's real
//! content arrives with its own later stage.

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};

use crate::perf::FrameStats;
use crate::theme;

pub struct AppShell {
    frame_stats: FrameStats,
}

impl AppShell {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            frame_stats: FrameStats::new(),
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
                    .child(div().flex_1().h_full().bg(theme::BG_APP)),
            )
    }
}
