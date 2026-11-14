//! The three-zone app shell: toolbar (two rows) + left panel + workspace,
//! mirroring Allodium-tauri's own layout convention (see its `CLAUDE.md`
//! "Views" section). Empty chrome only for Stage 1 — each zone's real
//! content arrives with its own later stage.

use std::sync::Arc;

use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};

use crate::audio::AudioEngine;
use crate::channel_rack::ChannelRack;
use crate::mixer::Mixer;
use crate::perf::FrameStats;
use crate::piano_roll::PianoRoll;
use crate::playlist::Playlist;
use crate::theme;

pub struct AppShell {
    frame_stats: FrameStats,
    channel_rack: Entity<ChannelRack>,
    piano_roll: Entity<PianoRoll>,
    playlist: Entity<Playlist>,
    mixer: Entity<Mixer>,
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
                    .child(
                        // Workspace: Channel Rack on top, Piano Roll below.
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .h(theme::CHANNEL_RACK_H)
                                    .w_full()
                                    .bg(theme::BG_APP)
                                    .border_b_1()
                                    .border_color(theme::BORDER)
                                    .child(self.channel_rack.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .w_full()
                                    .border_b_1()
                                    .border_color(theme::BORDER)
                                    .child(self.piano_roll.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .w_full()
                                    .border_b_1()
                                    .border_color(theme::BORDER)
                                    .child(self.playlist.clone()),
                            )
                            .child(div().h(theme::MIXER_H).w_full().child(self.mixer.clone())),
                    ),
            )
    }
}
