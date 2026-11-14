mod perf;
mod project;
mod shell;
mod theme;

use std::time::Duration;

use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};

use project::{Channel, ProjectFile};
use shell::AppShell;

/// Stage 2 persistence check: load (or create) a "default" project on
/// startup and log it to stderr. Run the app, confirm a test channel
/// gets created and saved, then run it again and confirm that same
/// channel comes back from disk unchanged — the actual "close and
/// reopen the app" verification the stage called for, not just a
/// passing unit test. Remove once Stage 3 (Channel Rack) gives this a
/// real UI instead of a stderr log.
fn verify_persistence() {
    match project::load_project("default") {
        Ok(Some(loaded)) => {
            eprintln!(
                "[persistence check] loaded existing project: {} channel(s), first = {:?}",
                loaded.channels.items.len(),
                loaded.channels.items.first().map(|c| (&c.name, c.volume, c.pan))
            );
        }
        Ok(None) => {
            let mut fresh = ProjectFile::default();
            fresh.channels.items.push(Channel {
                id: 1,
                name: "Test Channel".to_string(),
                sample_path: "samples/test.wav".to_string(),
                muted: false,
                soloed: false,
                pan: 0.5,
                volume: 0.8,
                mixer_track: 0,
            });
            match project::save_project("default", &fresh) {
                Ok(()) => eprintln!(
                    "[persistence check] no project found, created one with a test channel at {:?}",
                    project::projects_root()
                ),
                Err(e) => eprintln!("[persistence check] save failed: {e}"),
            }
        }
        Err(e) => eprintln!("[persistence check] load failed: {e}"),
    }
}

fn main() {
    verify_persistence();

    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    focus: true,
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| AppShell::new(window, cx)),
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
