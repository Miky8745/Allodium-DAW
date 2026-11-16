mod audio;
mod channel_rack;
mod color_utils;
mod file_browser;
mod floating_panel;
mod mixer;
mod perf;
mod piano_roll;
mod playlist;
mod preferences;
mod project;
mod shell;
mod theme;
mod toolbar;
mod widgets;

use std::sync::Arc;
use std::time::Duration;

use gpui::{App, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};

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

    // The full Lucide catalog, not just gpui-component's own ~100-icon
    // default bundle — the toolbar/panel-chrome redesign below draws names
    // (piano, list-music, sliders-vertical, circle-dot, …) outside that
    // smaller set.
    gpui::application().with_assets(gpui::assets::AllAssets).run(|cx: &mut App| {
        // Initializes gpui-component (and the gpui-base layer it sits on) —
        // required once, before any window that uses a styled component.
        // See `docs/stage-10-gpui-component.md`.
        gpui::init(cx);

        // gpui-component's default theme is light; switch to dark first
        // (its own dark palette — near-black backgrounds, neutral grays —
        // sits close enough to this app's own dark tones not to clash).
        // Then override just the handful of semantic slots this app's own
        // widgets will actually route through — primary/accent for
        // selected-or-active controls, danger for destructive ones,
        // background/border/radius for visual continuity with the hand-
        // styled containers — rather than reskinning all ~150 tokens the
        // library exposes. See `docs/stage-10-gpui-component.md`.
        gpui::component::Theme::change(gpui::component::ThemeMode::Dark, None, cx);
        // Pushes the default palette's colors into the handful of
        // gpui-component tokens this app's widgets route through. Preferences
        // → Theme calls the same function again at runtime to switch palettes
        // live — see `theme::apply_palette`.
        theme::apply_palette(cx, theme::default_palette());

        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    focus: true,
                    ..Default::default()
                },
                |window, cx| {
                    let app_shell = cx.new(|cx| AppShell::new(engine, window, cx));
                    cx.new(|cx| gpui::base::Root::new(app_shell, window, cx))
                },
            )
            .unwrap();
        cx.on_window_closed(|cx, _window_id| cx.quit()).detach();
        cx.activate(true);

        // Workaround for a GPUI/Wayland presentation bug on this machine
        // (NVIDIA + niri): the Vulkan swapchain isn't (re)created until the
        // window's content size actually changes, so nothing presents on
        // screen (and the render loop never ticks again) until the first
        // resize. Nudging the size by 1px and back right after launch
        // forces that swapchain recreation — originally confirmed in
        // Allodium-GPUI-test; see this repo's CLAUDE.md. After switching to
        // gpui-component (`docs/stage-10-gpui-component.md`), the grow and
        // shrink calls had to move into two separate `await` steps instead
        // of back-to-back in one synchronous closure — doing both
        // instantly no longer reaches the compositor as a real resize on
        // this backend, it just gets coalesced away.
        window
            .update(cx, |_, window, cx| {
                window
                    .spawn(cx, async move |cx| {
                        cx.background_executor().timer(Duration::from_millis(200)).await;
                        let mut nudged = bounds.size;
                        nudged.width += px(1.0);
                        let _ = cx.update(|window, _| window.resize(nudged));
                        cx.background_executor().timer(Duration::from_millis(200)).await;
                        let _ = cx.update(|window, _| window.resize(bounds.size));
                    })
                    .detach();
            })
            .ok();
    });
}
