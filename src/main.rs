mod audio;
mod perf;
mod project;
mod shell;
mod theme;

use std::sync::Arc;
use std::time::Duration;

use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};

use audio::{AudioEngine, ChannelVoiceData, PlaybackSnapshot, SampleBuffer};
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

/// Stage 3 scheduler/mixing check: play a synthesized 440Hz test tone on
/// a 4-on-the-floor 16-step pattern at 120 BPM for a few seconds, log
/// `current_step()` periodically to confirm it advances steadily instead
/// of jumping, then measure 20 stop/play restart cycles ("restarting
/// playback should be cheap" — the stage explicitly called for measuring
/// this, not assuming it). No hardcoded sample file from any other repo
/// on this machine — set `ALLODIUM_TEST_SAMPLE=/path/to/file` to decode
/// a real file through `decode_sample` instead, for a one-off manual
/// check; the committed default is self-contained. Runs on its own
/// thread so it doesn't delay the window opening. Remove once a real
/// transport UI replaces this stderr log.
fn verify_audio_engine() {
    std::thread::spawn(|| {
        let sample = match std::env::var("ALLODIUM_TEST_SAMPLE") {
            Ok(path) => match audio::decode_sample(std::path::Path::new(&path)) {
                Ok(s) => {
                    eprintln!(
                        "[audio check] decoded {path}: {} ch @ {}Hz, {} frames",
                        s.channels,
                        s.sample_rate,
                        s.frame_count()
                    );
                    s
                }
                Err(e) => {
                    eprintln!("[audio check] failed to decode {path}: {e}, falling back to synthesized tone");
                    synthesize_test_tone()
                }
            },
            Err(_) => synthesize_test_tone(),
        };

        let snapshot = PlaybackSnapshot {
            channels: vec![ChannelVoiceData {
                sample,
                steps: (0..16).map(|i| i % 4 == 0).collect(),
                notes: vec![],
                volume: 0.5,
                pan: 0.5,
                muted: false,
            }],
            pattern_length_steps: 16,
        };

        let mut engine = AudioEngine::new();
        if let Err(e) = engine.play(snapshot, 120.0) {
            eprintln!("[audio check] play failed: {e}");
            return;
        }
        eprintln!("[audio check] playing a 4-on-the-floor test pattern at 120 BPM for 3s");
        for _ in 0..6 {
            std::thread::sleep(Duration::from_millis(500));
            eprintln!("[audio check] current_step = {:.2}", engine.current_step());
        }
        engine.stop();

        // Measure restart cost directly rather than assuming it's cheap.
        let sample_for_restart = match std::env::var("ALLODIUM_TEST_SAMPLE") {
            Ok(path) => audio::decode_sample(std::path::Path::new(&path)).unwrap_or_else(|_| synthesize_test_tone()),
            Err(_) => synthesize_test_tone(),
        };
        let restarts = 20;
        let start = std::time::Instant::now();
        for _ in 0..restarts {
            let snapshot = PlaybackSnapshot {
                channels: vec![ChannelVoiceData {
                    sample: sample_for_restart.clone(),
                    steps: vec![true; 16],
                    notes: vec![],
                    volume: 0.0, // silent for the timing measurement
                    pan: 0.5,
                    muted: false,
                }],
                pattern_length_steps: 16,
            };
            let _ = engine.play(snapshot, 120.0);
            engine.stop();
        }
        let elapsed = start.elapsed();
        eprintln!(
            "[audio check] {restarts} play/stop restarts took {:.1}ms total, {:.2}ms/restart",
            elapsed.as_secs_f64() * 1000.0,
            elapsed.as_secs_f64() * 1000.0 / restarts as f64
        );
    });
}

fn synthesize_test_tone() -> Arc<SampleBuffer> {
    const SAMPLE_RATE: u32 = 48000;
    const FREQ_HZ: f32 = 440.0;
    const DURATION_SECS: f32 = 0.3;
    let n = (SAMPLE_RATE as f32 * DURATION_SECS) as usize;
    let data = (0..n)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            (t * FREQ_HZ * std::f32::consts::TAU).sin() * 0.6
        })
        .collect();
    Arc::new(SampleBuffer { channels: 1, sample_rate: SAMPLE_RATE, data })
}

fn main() {
    verify_persistence();
    verify_audio_engine();

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
