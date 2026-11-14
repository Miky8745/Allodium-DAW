mod audio;
mod channel_rack;
mod mixer;
mod perf;
mod piano_roll;
mod playlist;
mod project;
mod shell;
mod theme;

use std::sync::Arc;
use std::time::Duration;

use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};

use audio::AudioEngine;
use shell::AppShell;

fn main() {
    let engine = match AudioEngine::new() {
        Ok(e) => Arc::new(e),
        Err(e) => {
            eprintln!("[audio] failed to open an output device: {e}");
            return;
        }
    };

    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    focus: true,
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| AppShell::new(engine, window, cx)),
            )
            .unwrap();
        cx.on_window_closed(|cx| cx.quit()).detach();
        cx.activate(true);

        // Workaround for a GPUI/Wayland presentation bug on this machine
        // (NVIDIA + niri): the Vulkan swapchain isn't (re)created until the
        // window's content size actually changes, so nothing presents on
        // screen until the first resize. Nudging the size by 1px and back
        // right after launch forces that swapchain recreation. Confirmed
        // necessary in Allodium-GPUI-test; see this repo's CLAUDE.md.
        window
            .update(cx, |_, window, cx| {
                window
                    .spawn(cx, async move |cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(200))
                            .await;
                        let _ = cx.update(|window, _| {
                            let mut nudged = bounds.size;
                            nudged.width += px(1.0);
                            window.resize(nudged);
                            window.resize(bounds.size);
                        });
                    })
                    .detach();
            })
            .ok();
    });
}
