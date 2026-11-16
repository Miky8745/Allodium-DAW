//! Row 1 + row 2 toolbar — ports every control from Allodium-tauri's
//! `Toolbar.svelte` into this app's own visual language (see
//! `docs/stage-09-toolbar.md`). Icons/labels/positions were freely
//! redesigned, but every original control has a place somewhere here:
//!
//! - Transport (Pattern/Song switch, Play/Stop/Pause/Record, BPM, step
//!   position), the four feature toggles, and the two monitor widgets
//!   sit in row 1, right of the File/Edit/… menu.
//! - Panel toggles, Auto-scroll/one-click-record, Snap/Pattern pickers,
//!   the MIDI LED, and the Shift/Alt/Ctrl badges sit in row 2.
//!
//! Visual pass: every control used to be an identically-sized, identically-
//! weighted glyph square (ASCII/Unicode approximations of icons, no two
//! quite the same optical weight) — the "flat ImGUI toolbox" look this
//! redesign was asked to fix. This version draws real vector icons (the
//! bundled Lucide catalog, via `gpui::assets::AllAssets` registered in
//! `main.rs`) and gives the transport row actual hierarchy: Play is the one
//! large, bordered, filled-when-active control; everything else is a small
//! quiet icon toggle. Record is the one control that departs from the
//! generic checked-state accent (it recolors to `theme::warm_accent()`,
//! matching the mute/record "warm = destructive" convention in `theme.rs`)
//! since `gpui::component::button::Toggle`'s own pressed style is hardcoded
//! to the theme's cool accent — hand-styled rather than ported through that
//! component for this one case. Row 2 sits on a darker fill than row 1, so
//! it reads as the quieter, secondary strip it actually is.
//!
//! Wiring fidelity: anything this rewrite's audio engine already backs
//! (play/stop, tempo, step position, Pattern/Song mode, pattern
//! selection, panel visibility) is wired live. Several of the original's
//! own toggles (Record, Precount, Loop record, Auto-scroll, One-click
//! record, Snapping) were themselves just local component state in
//! `Toolbar.svelte` with no further consumption shown there either — those
//! are ported at that same fidelity: a real, persistent, click-to-toggle
//! control, just not driving a scheduler/audio feature that doesn't
//! exist yet in this rewrite. Metronome goes one step further and ports
//! the original's one real side effect (turning it on auto-starts
//! playback). Only two controls have no faithful equivalent at all and
//! stay inert, disabled chrome: the MIDI LED (no MIDI I/O anywhere in
//! this codebase) and the Explorer panel toggle (no file-browser panel
//! exists yet — MVP item #8, still unbuilt).

use std::sync::Arc;

use gpui::assets::IconName;
use gpui::component::button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants};
use gpui::component::input::Input;
use gpui::component::menu::{DropdownMenu, PopupMenuItem};
use gpui::component::separator::Separator;
use gpui::component::{Disableable, Icon, Sizable, WindowExt};
use gpui::{
    BoxShadow, ClickEvent, Context, Hsla, IntoElement, MouseButton, MouseDownEvent, Modifiers,
    ParentElement, ScrollEvent, Styled, Window, div, point, prelude::*, px,
};
use rustfft::num_complex::Complex32;

use crate::audio;
use crate::floating_panel::PanelKind;
use crate::preferences;
use crate::shell::AppShell;
use crate::theme;

const MENU_LABELS: [&str; 6] = ["File", "Edit", "View", "Options", "Tools", "Help"];

// Audio-monitor envelope constants — pulled directly from Allodium-tauri's
// own `meterMath.ts` (`RELEASE`, `MIN_DB`) and `BlackBox.svelte`
// (`WAVE_RELEASE`, `WAVE_GAIN_FLOOR`), not re-tuned guesses. Both apps
// apply these once per UI frame, not per audio sample — an earlier
// version here did the peak decay at audio-sample rate inside
// `MeterState` itself, which (at 44.1kHz) decayed a per-sample attack to
// near-zero between any two render polls, reading as "barely moves."
const PEAK_RELEASE: f32 = 0.85;
const PEAK_MIN_DB: f32 = -48.0;
const WAVE_RELEASE: f32 = 0.965;
const WAVE_GAIN_FLOOR: f32 = 0.04;
// Same autoscale shape as the oscilloscope's wave_gain (fast-attack/slow-
// release against this frame's own peak bin — `BlackBox.svelte`'s
// `drawSpectrum` comment: "a quiet passage still uses the full height").
// The floor is this module's own calibration, not ported: the original's
// `FREQ_GAIN_FLOOR = 24` is a *byte*-scale (`getByteFrequencyData`, 0..255)
// floor, meaningless against `spectrum_magnitude`'s own linear-amplitude
// units below.
const FREQ_RELEASE: f32 = 0.965;
const FREQ_GAIN_FLOOR: f32 = 0.02;
const SPECTRUM_BARS: usize = 44;

pub struct ToolbarState {
    /// Name/value of the last-hovered toolbar control — mirrors the
    /// original's `hoverInfoStore`, scoped to this toolbar's own controls
    /// rather than the whole app's Dial/Knob/Slider widgets. Stays
    /// showing after the mouse moves off; blank until anything's been
    /// hovered at all.
    pub(crate) hover: Option<(&'static str, String)>,
    pub(crate) modifiers: Modifiers,
    paused: bool,
    recording: bool,
    typing_keyboard: bool,
    precount: bool,
    loop_record: bool,
    metronome: bool,
    auto_scroll: bool,
    one_click_record: bool,
    snap_quarter: bool,
    /// Whether the BPM pill is showing `shell::AppShell::tempo_input`
    /// (double-click to type an exact value) instead of the chevron/value
    /// display.
    pub(crate) editing_tempo: bool,
    /// Detects an actual change in Preferences' monitor-source setting
    /// (checked once per render, see `render_toolbar`) so
    /// `ChannelRack::{start,stop}_system_capture` fires exactly on the
    /// transition, not every frame.
    last_monitor_source: Option<preferences::MonitorSource>,
    /// The peak meter's attack/release envelope, applied here at render
    /// cadence against `ChannelRack::peak_levels`' raw per-frame max —
    /// Allodium-tauri's own split (`meterMath.ts`'s `peakOf` is that same
    /// raw max; `PeakMeter.svelte`'s own `requestAnimationFrame` loop
    /// applies the envelope). Reset to 0 on a monitor-source switch.
    peak_env: (f32, f32),
    /// The oscilloscope's autoscale ceiling — Allodium-tauri's own
    /// `BlackBox.svelte` `waveGain`: fast-attack to this frame's own peak
    /// deviation, slow release otherwise, so the trace fills the display
    /// regardless of how quiet the source actually is. Reset to the gain
    /// floor on a monitor-source switch ("switching source rescales from
    /// scratch", ported verbatim from that file's own comment).
    wave_gain: f32,
    /// Which of the two views the audio monitor shows — click toggles it
    /// (`Toolbar.svelte`'s own `BlackBox` behavior: "click for spectrum
    /// analyzer" / "click for oscilloscope").
    monitor_mode: MonitorMode,
    /// `spectrogram`'s autoscale ceiling — the spectrum-analyzer
    /// equivalent of `wave_gain`, see `FREQ_RELEASE`/`FREQ_GAIN_FLOOR`.
    freq_gain: f32,
    /// Built once (planning an FFT has real setup cost — re-running it
    /// every render, dozens of times a second, would be wasted work) and
    /// reused by every `spectrogram` call.
    fft: Arc<dyn rustfft::Fft<f32>>,
    /// Throttles how often `spectrogram`'s FFT actually re-runs (every
    /// `SPECTRUM_UPDATE_EVERY`th render) — measured on this machine: an
    /// *idle* 1024-point FFT call costs single-digit microseconds by its
    /// own clock, yet having run it at all somehow costs the next ~60ms
    /// of wall time overall (confirmed by isolating every other stage;
    /// none of them account for the gap). The closest explanation is a
    /// SIMD frequency/power-state transition tax that isn't visible to
    /// `Instant` timing around the call itself — a known class of issue
    /// with AVX-heavy code on some CPUs, not something `rustfft`'s own
    /// numbers being fast rules out. A ~15-20Hz update rate (rather than
    /// recomputing every render) is still visually live for a spectrum
    /// display and cuts however-many-times-a-second this cost is paid
    /// without changing anything about the computation itself.
    spectrum_frame: u32,
    cached_spectrum_bins: Vec<f32>,
}

const SPECTRUM_UPDATE_EVERY: u32 = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MonitorMode {
    Oscilloscope,
    Spectrum,
}

impl Default for ToolbarState {
    fn default() -> Self {
        Self {
            hover: None,
            modifiers: Modifiers::default(),
            paused: false,
            recording: false,
            typing_keyboard: false,
            precount: false,
            loop_record: false,
            metronome: false,
            auto_scroll: false,
            one_click_record: false,
            snap_quarter: true, // matches the original's default `snapping = '1/4'`
            editing_tempo: false,
            last_monitor_source: None,
            peak_env: (0.0, 0.0),
            wave_gain: WAVE_GAIN_FLOOR,
            monitor_mode: MonitorMode::Oscilloscope,
            freq_gain: FREQ_GAIN_FLOOR,
            fft: rustfft::FftPlanner::new().plan_fft_forward(audio::FFT_LEN),
            spectrum_frame: 0,
            cached_spectrum_bins: vec![0.0; SPECTRUM_BARS],
        }
    }
}

/// A small borderless icon toggle, checked state tinted with `color`
/// rather than the generic theme accent — used for the feature-toggle
/// cluster (`theme::secondary()`, a deliberately different hue from the
/// amber transport row) so that whole group reads as its own role, not
/// just a quieter copy of Play/Pause/Record. Covers every ordinary
/// toggle-shaped control except Play/Stop/Pause/Record (their own
/// bordered `transport_btn`).
fn toggle(
    id: impl Into<gpui::ElementId>,
    icon: IconName,
    active: bool,
    color: Hsla,
    name: &'static str,
    value: impl Into<String>,
    on_click: impl Fn(&mut AppShell, &gpui::ClickEvent, &mut Window, &mut Context<AppShell>) + 'static,
    cx: &mut Context<AppShell>,
) -> impl IntoElement {
    let id: gpui::ElementId = id.into();
    let value = value.into();
    div()
        .id(id.clone())
        .w(px(28.0))
        .h(px(26.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::RADIUS_XS)
        .cursor_pointer()
        .text_color(if active { color } else { theme::BTN_TEXT })
        .when(active, |d| d.bg(color.alpha(0.1)).shadow(glow(color)))
        .hover(|s| if active { s } else { s.bg(theme::BTN_HOVER).text_color(theme::MAIN_TEXT) })
        .child(Icon::new(icon).small())
        .on_click(cx.listener(on_click))
        .on_hover(cx.listener(move |shell, hovering: &bool, _, cx| {
            if *hovering {
                shell.toolbar.hover = Some((name, value.clone()));
                cx.notify();
            }
        }))
}

/// A soft colored glow for an active/filled control — a single wide,
/// faint `BoxShadow`, the nearest GPUI equivalent to the mockup's
/// `box-shadow: 0 0 Npx <color>` (GPUI has no backdrop-filter/blur, but
/// an ordinary shadow with a large blur radius and no offset reads the
/// same way: a soft halo, not a directional drop shadow).
pub(crate) fn glow(color: Hsla) -> Vec<BoxShadow> {
    vec![BoxShadow { color: color.alpha(0.35), offset: point(px(0.0), px(0.0)), blur_radius: px(10.0), spread_radius: px(0.0), inset: false }]
}

/// One of the four uniform bordered transport buttons (Play/Stop/Pause/
/// Record) — Allodium-tauri's own `.tb-xp` treatment: a bordered square,
/// neutral at rest, accent-bordered-and-glowing while active. `color` is
/// the active-state accent (`theme::accent()` for Play/Pause,
/// `theme::warm_accent()` for Record so recording reads as categorically
/// different from an ordinary engaged toggle); Stop has no active state
/// at all, so its caller just passes `active: false`.
fn transport_btn(
    id: impl Into<gpui::ElementId>,
    icon: IconName,
    active: bool,
    color: Hsla,
    name: &'static str,
    value: impl Into<String>,
    on_click: impl Fn(&mut AppShell, &gpui::ClickEvent, &mut Window, &mut Context<AppShell>) + 'static,
    cx: &mut Context<AppShell>,
) -> impl IntoElement {
    let id: gpui::ElementId = id.into();
    let value = value.into();
    div()
        .id(id.clone())
        .w(px(34.0))
        .h(px(34.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::RADIUS_SM)
        .border_1()
        .cursor_pointer()
        .bg(if active { color.alpha(0.09) } else { theme::BTN_BG })
        .border_color(if active { color } else { theme::BTN_BORDER })
        .text_color(if active { color } else { theme::BTN_TEXT })
        .when(active, |d| d.shadow(glow(color)))
        .hover(|s| if active { s } else { s.bg(theme::BTN_HOVER).text_color(theme::MAIN_TEXT) })
        .child(Icon::new(icon).small())
        .on_click(cx.listener(on_click))
        .on_hover(cx.listener(move |shell, hovering: &bool, _, cx| {
            if *hovering {
                shell.toolbar.hover = Some((name, value.clone()));
                cx.notify();
            }
        }))
}

/// A disabled toggle with no backend at all (MIDI LED, the Explorer
/// panel) — its hover-info note says so honestly rather than implying
/// it works.
fn inert(id: impl Into<gpui::ElementId>, icon: IconName, name: &'static str, note: &'static str, cx: &mut Context<AppShell>) -> impl IntoElement {
    let id: gpui::ElementId = id.into();
    div()
        .id(id.clone())
        .on_hover(cx.listener(move |shell, hovering: &bool, _, cx| {
            if *hovering {
                shell.toolbar.hover = Some((name, note.to_string()));
                cx.notify();
            }
        }))
        .child(Toggle::new(id).w(px(28.0)).h(px(26.0)).disabled(true).child(Icon::new(icon).small()))
}

/// A tiny click target that nudges the tempo by `delta` BPM — the
/// discoverable counterpart to the tempo pill's scroll-to-adjust gesture.
fn tempo_step_btn(id: impl Into<gpui::ElementId>, icon: IconName, delta: f64, cx: &mut Context<AppShell>) -> impl IntoElement {
    div()
        .id(id.into())
        .w(px(14.0))
        .h(px(14.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::RADIUS_XS)
        .cursor_pointer()
        .text_color(theme::accent_dim())
        .hover(|s| s.bg(theme::BTN_HOVER).text_color(theme::accent()))
        .child(Icon::new(icon).xsmall())
        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
            let current = this.channel_rack.read(cx).tempo();
            this.channel_rack.update(cx, |cr, cx| cr.set_tempo(current + delta, cx));
        }))
}

/// The toolbar's audio monitor — a real-time polyline of the master
/// mix's actual waveform (`ChannelRack::scope_snapshot`), not the
/// placeholder "no hook yet" chrome this rewrite shipped with before.
/// The original's second mode (an FFT spectrum analyzer, toggled by
/// clicking the same control) isn't implemented — that needs an FFT
/// dependency and bucketing logic this pass didn't take on — so this
/// only ever shows the oscilloscope.
fn oscilloscope(samples: Vec<f32>) -> impl IntoElement {
    // A bar waveform (per-chunk peak amplitude), not a traced polyline.
    // The first version used `canvas` + `PathBuilder::stroke` to draw an
    // actual connected line, which measured fine on paper but cost ~100ms/
    // frame on this machine's GPU backend once it was actually painting
    // every frame (continuous redraw already running for the playhead) —
    // tanked the whole app from ~35fps to ~7fps. Plain divs, the same
    // approach every other per-frame-redrawn view in this app already
    // uses (step grids, pattern blocks), measured back at baseline.
    const BARS: usize = 22;
    let chunk = (samples.len() / BARS).max(1);
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(1.0))
        .w(px(90.0))
        .h(px(30.0))
        .children((0..BARS).map(|i| {
            let start = (i * chunk).min(samples.len());
            let end = (start + chunk).min(samples.len());
            let peak = samples.get(start..end).map(|s| s.iter().fold(0.0f32, |m, v| m.max(v.abs()))).unwrap_or(0.0);
            let h = (peak.clamp(0.0, 1.0) * 26.0).max(2.0);
            div().w(px(2.0)).h(px(h)).rounded(px(1.0)).bg(theme::accent())
        }))
}

/// Two dBFS-scaled bars — real master-bus levels
/// (`ChannelRack::peak_levels`), same metering source as the
/// oscilloscope above. `theme::METER_GREEN`/`METER_YELLOW` existed since
/// the original visual pass, reserved for exactly this; this is what
/// finally uses them.
fn peak_meter(levels: (f32, f32)) -> impl IntoElement {
    const BAR_W: f32 = 44.0;
    let bar = move |level: f32| {
        // Linear amplitude bunches every normal listening level up near
        // the left edge; a rough dBFS mapping (-48dBFS..0dBFS -> 0..1)
        // spreads them across the bar the way a real meter reads.
        let db = lin_to_db(level);
        let frac = ((db - PEAK_MIN_DB) / -PEAK_MIN_DB).clamp(0.0, 1.0);
        let color = if frac > 0.92 {
            theme::destructive()
        } else if frac > 0.75 {
            theme::METER_YELLOW
        } else {
            theme::METER_GREEN
        };
        div()
            .w(px(BAR_W))
            .h(px(5.0))
            .rounded(theme::RADIUS_XS)
            .bg(theme::BTN_BG)
            .overflow_hidden()
            .child(div().h_full().w(px(BAR_W * frac)).bg(color))
    };
    div().flex().flex_col().gap(px(3.0)).child(bar(levels.0)).child(bar(levels.1))
}

fn lin_to_db(level: f32) -> f32 {
    20.0 * level.max(1e-4).log10()
}

/// Real linear-amplitude magnitude per positive-frequency FFT bin (length
/// `samples.len() / 2`, DC..Nyquist) — a Hann-windowed (reduces spectral
/// leakage into neighboring bins) forward FFT via the cached plan in
/// `fft`, normalized by `len/2` so a full-scale sinusoid's own bin reads
/// back out at roughly its original time-domain amplitude (the same
/// units `wave_gain`'s floor is already calibrated against).
fn spectrum_magnitude(samples: &[f32], fft: &dyn rustfft::Fft<f32>) -> Vec<f32> {
    let n = samples.len();
    let mut buf: Vec<Complex32> = samples
        .iter()
        .enumerate()
        .map(|(i, s)| {
            // Hann window.
            let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos();
            Complex32::new(s * w, 0.0)
        })
        .collect();
    fft.process(&mut buf);
    let norm = n as f32 / 2.0;
    buf[..n / 2].iter().map(|c| c.norm() / norm).collect()
}

/// Maps bar `i` of `bar_count` onto a bin index, exponentially rather
/// than linearly — Allodium-tauri's own `binEdge`, ported verbatim:
/// "exponential bucket edges so low frequencies (where most musical
/// energy sits) get more than the first pixel or two of a linear
/// bin->pixel map."
fn bin_edge(bar: usize, bar_count: usize, bins: usize) -> usize {
    let frac = bar as f32 / bar_count as f32;
    (bins as f32).powf(frac).round().clamp(1.0, (bins - 1) as f32) as usize
}

/// The spectrum-analyzer view — many thin bars (not a fixed handful),
/// so with a finite number of pixels to spend it reads as continuous
/// rather than block-like, the same way the oscilloscope's own bars do.
fn spectrogram(bins: Vec<f32>) -> impl IntoElement {
    let n = bins.len();
    let bar_w = px(90.0 / SPECTRUM_BARS as f32);
    div()
        .flex()
        .items_center()
        .justify_center()
        .w(px(90.0))
        .h(px(30.0))
        .children((0..SPECTRUM_BARS).map(|i| {
            let lo = bin_edge(i, SPECTRUM_BARS, n);
            let hi = bin_edge(i + 1, SPECTRUM_BARS, n).max(lo + 1).min(n);
            let peak = bins.get(lo..hi).map(|s| s.iter().cloned().fold(0.0f32, f32::max)).unwrap_or(0.0);
            let h = (peak.clamp(0.0, 1.0) * 26.0).max(2.0);
            div().w(bar_w).h(px(h)).bg(theme::accent())
        }))
}

fn vsep() -> impl IntoElement {
    Separator::vertical().h(px(22.0)).w(px(1.0)).mx_2()
}

/// The one real menu in the File/Edit/View/… row — a dropdown with a
/// single "Preferences…" item that opens the in-app Preferences dialog
/// (`src/preferences.rs`). The rest of the row stays inert text (no
/// backend behind them yet), so only this one label gets the quiet
/// ghost-button treatment that makes it look (and act) clickable.
fn edit_menu(prefs: gpui::Entity<preferences::Preferences>) -> impl IntoElement {
    Button::new("menu-edit")
        .ghost()
        .compact()
        .text_size(px(11.0))
        .text_color(theme::accent_dim().alpha(0.7))
        .label("Edit")
        .dropdown_menu(move |menu, _, _| {
            let prefs = prefs.clone();
            menu.item(PopupMenuItem::new("Preferences…").icon(IconName::Settings).on_click(move |_, window, cx| {
                let prefs = prefs.clone();
                window.open_dialog(cx, move |dialog, _, cx| dialog.title("Preferences").w(px(520.0)).child(preferences::render(&prefs, cx)));
            }))
        })
}

impl AppShell {
    fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        let playing = self.channel_rack.read(cx).is_playing();
        if playing {
            self.channel_rack.update(cx, |cr, cx| cr.toggle_play(cx));
            self.toolbar.paused = true;
        } else if self.toolbar.paused {
            self.channel_rack.update(cx, |cr, cx| cr.toggle_play(cx));
            self.toolbar.paused = false;
        }
        cx.notify();
    }

    /// Ports the original's one real side effect of flipping this
    /// switch: turning the metronome on auto-starts playback if it
    /// isn't already running (there's no audible click yet — no
    /// metronome-tick scheduling exists in the audio engine).
    fn toggle_metronome(&mut self, cx: &mut Context<Self>) {
        self.toolbar.metronome = !self.toolbar.metronome;
        if self.toolbar.metronome && !self.channel_rack.read(cx).is_playing() {
            self.channel_rack.update(cx, |cr, cx| cr.toggle_play(cx));
        }
        cx.notify();
    }

    pub(crate) fn render_toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let cr = self.channel_rack.read(cx);
        let playing = cr.is_playing();
        let song_mode = cr.is_song_mode();
        let tempo = cr.tempo();
        let step = (if song_mode { cr.song_step() } else { cr.current_step() }).floor() + 1.0;
        let patterns: Vec<(u32, String)> = cr.patterns().iter().map(|p| (p.id, p.name.clone())).collect();
        let selected_pattern_id = cr.selected_pattern_id();
        let pattern_label = patterns
            .iter()
            .find(|(id, _)| *id == selected_pattern_id)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| "—".to_string());
        // Preferences → General → "Oscilloscope / peak meter source" — the
        // original's `MonitorSource` ('app' | 'system'). `System` is real
        // capture (`audio::system_capture`, a `pw-record` child process),
        // started/stopped here exactly on the transition (not every
        // frame) by comparing against the last render's value.
        let monitor_source = self.preferences.read(cx).monitor_source();
        if self.toolbar.last_monitor_source != Some(monitor_source) {
            self.toolbar.last_monitor_source = Some(monitor_source);
            match monitor_source {
                preferences::MonitorSource::Daw => cr.stop_system_capture(),
                preferences::MonitorSource::System => cr.start_system_capture(),
            }
            // "Switching source rescales from scratch" — Allodium-tauri's
            // own `BlackBox.svelte` comment, ported along with the reset
            // itself: a ceiling built up against the old source shouldn't
            // carry over and make the new one look falsely quiet (or let
            // a stale low ceiling make it clip).
            self.toolbar.peak_env = (0.0, 0.0);
            self.toolbar.wave_gain = WAVE_GAIN_FLOOR;
            self.toolbar.freq_gain = FREQ_GAIN_FLOOR;
        }
        let (raw_scope, raw_peak, monitor_error) = match monitor_source {
            preferences::MonitorSource::Daw => (cr.scope_snapshot(), cr.peak_levels(), None),
            preferences::MonitorSource::System => (cr.system_scope_snapshot(), cr.system_peak_levels(), cr.system_capture_error()),
        };
        // Both envelopes applied here, once per render, against the raw
        // per-frame max `ChannelRack::{,system_}peak_levels`/
        // `{,system_}scope_snapshot` now report — see `MeterState::
        // take_frame_peak`'s doc comment for why this moved from the
        // audio thread to here.
        self.toolbar.peak_env.0 = if raw_peak.0 > self.toolbar.peak_env.0 { raw_peak.0 } else { self.toolbar.peak_env.0 * PEAK_RELEASE };
        self.toolbar.peak_env.1 = if raw_peak.1 > self.toolbar.peak_env.1 { raw_peak.1 } else { self.toolbar.peak_env.1 * PEAK_RELEASE };
        let peak_levels = self.toolbar.peak_env;

        let monitor_mode = self.toolbar.monitor_mode;
        // Only ever compute the view that's actually showing — an FFT is
        // cheap, but there's no reason to pay for it every frame while
        // the oscilloscope is the one on screen.
        let monitor_view = match monitor_mode {
            MonitorMode::Oscilloscope => {
                let frame_gain = raw_scope.iter().fold(0.0f32, |m, v| m.max(v.abs()));
                self.toolbar.wave_gain = if frame_gain > self.toolbar.wave_gain { frame_gain } else { self.toolbar.wave_gain * WAVE_RELEASE };
                let wave_scale = 1.0 / self.toolbar.wave_gain.max(WAVE_GAIN_FLOOR);
                let scope_samples: Vec<f32> = raw_scope.iter().map(|s| (s * wave_scale).clamp(-1.0, 1.0)).collect();
                oscilloscope(scope_samples).into_any_element()
            }
            MonitorMode::Spectrum => {
                // The FFT itself measures in the tens of microseconds, yet
                // actually running it visibly costs the rest of that
                // frame's paint time (unconfirmed root cause — see
                // `cached_spectrum_bins`'s doc comment). Throttling how
                // often it runs, and reusing the last result on the
                // skipped frames, keeps the spectrum view responsive
                // without needing that root cause nailed down.
                self.toolbar.spectrum_frame = self.toolbar.spectrum_frame.wrapping_add(1);
                if self.toolbar.spectrum_frame % SPECTRUM_UPDATE_EVERY == 0 {
                    let raw_fft = match monitor_source {
                        preferences::MonitorSource::Daw => cr.fft_snapshot(),
                        preferences::MonitorSource::System => cr.system_fft_snapshot(),
                    };
                    let magnitudes = spectrum_magnitude(&raw_fft, self.toolbar.fft.as_ref());
                    let frame_gain = magnitudes.iter().cloned().fold(0.0f32, f32::max);
                    self.toolbar.freq_gain = if frame_gain > self.toolbar.freq_gain { frame_gain } else { self.toolbar.freq_gain * FREQ_RELEASE };
                    let fgain = self.toolbar.freq_gain.max(FREQ_GAIN_FLOOR);
                    self.toolbar.cached_spectrum_bins = magnitudes.iter().map(|m| (m / fgain).clamp(0.0, 1.0)).collect();
                }
                spectrogram(self.toolbar.cached_spectrum_bins.clone()).into_any_element()
            }
        };
        // Shared by both the scope's and peak meter's hover tooltips below
        // — the only difference between them while on System audio.
        let monitor_system_label = match &monitor_error {
            Some(e) => format!("System audio — capture failed: {e}"),
            None => "System audio (pw-record loopback of the default sink)".to_string(),
        };
        let modifiers = self.toolbar.modifiers;
        let hover = self.toolbar.hover.clone();
        let snap_quarter = self.toolbar.snap_quarter;

        div()
            .w_full()
            .flex()
            .flex_col()
            .bg(theme::BG_APP)
            .border_b_1()
            .border_color(theme::BORDER)
            .child(
                // ── Row 1: menu, transport, BPM/step, feature toggles, monitors ──
                div()
                    .h(theme::TOOLBAR_ROW_H)
                    .w_full()
                    .flex()
                    .items_center()
                    .child(
                        // The one column with its own fill (Allodium-tauri's
                        // `.tb-menu`) — everything to its right sits directly
                        // on the toolbar's own `BG_APP`, no separate panel fill.
                        div()
                            .h_full()
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_3()
                            .bg(theme::SLATE)
                            .border_r_1()
                            .border_color(theme::BORDER)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(Icon::new(IconName::AudioLines).small().text_color(theme::accent()))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(theme::accent())
                                            .child("Allodium"),
                                    ),
                            )
                            .children(MENU_LABELS.map(|label| {
                                if label == "Edit" {
                                    edit_menu(self.preferences.clone()).into_any_element()
                                } else {
                                    div().text_size(px(11.0)).text_color(theme::BTN_TEXT).child(label).into_any_element()
                                }
                            })),
                    )
                    .child(vsep())
                    .child(
                        // Pattern / Song mode — one segmented control, not two
                        // separately-boxed pills: the design guide's rule that
                        // a segmented control shares one silhouette.
                        ToggleGroup::new("mode")
                            .outline()
                            .segmented()
                            .single()
                            .small()
                            .child(Toggle::new("mode-pat").label("PAT").checked(!song_mode))
                            .child(Toggle::new("mode-song").label("SONG").checked(song_mode))
                            .on_click(cx.listener(|this, checks: &Vec<bool>, _, cx| {
                                let want_song = checks.get(1).copied().unwrap_or(false);
                                if want_song != this.channel_rack.read(cx).is_song_mode() {
                                    this.channel_rack.update(cx, |cr, cx| cr.toggle_song_mode(cx));
                                }
                            })),
                    )
                    .child(vsep())
                    .child(
                        // Transport: Play · Stop · Pause · Record — four
                        // uniform bordered squares (Allodium-tauri's `.tb-xp`),
                        // not a hierarchy of one big button and three small
                        // ones. Record alone recolors to the warm accent
                        // instead of the cool one while active.
                        div()
                            .flex()
                            .items_center()
                            .gap(px(3.0))
                            .child(transport_btn(
                                "transport-play",
                                IconName::Play,
                                playing,
                                theme::accent(),
                                "Play",
                                if playing { "Playing" } else { "Stopped" },
                                |this, _, _, cx| this.channel_rack.update(cx, |cr, cx| cr.toggle_play(cx)),
                                cx,
                            ))
                            .child(transport_btn(
                                "transport-stop",
                                IconName::Square,
                                false,
                                theme::accent(),
                                "Stop",
                                "",
                                |this, _, _, cx| {
                                    if this.channel_rack.read(cx).is_playing() {
                                        this.channel_rack.update(cx, |cr, cx| cr.toggle_play(cx));
                                    }
                                    this.toolbar.paused = false;
                                },
                                cx,
                            ))
                            .child(transport_btn(
                                "transport-pause",
                                IconName::Pause,
                                self.toolbar.paused,
                                theme::accent(),
                                "Pause",
                                if self.toolbar.paused { "Paused" } else { "Not paused" },
                                |this, _, _, cx| this.toggle_pause(cx),
                                cx,
                            ))
                            .child(transport_btn(
                                "transport-record",
                                IconName::CircleDot,
                                self.toolbar.recording,
                                theme::warm_accent(),
                                "Record",
                                if self.toolbar.recording { "Recording" } else { "Idle" },
                                |this, _, _, cx| {
                                    this.toolbar.recording = !this.toolbar.recording;
                                    cx.notify();
                                },
                                cx,
                            )),
                    )
                    .child(vsep())
                    .child({
                        // Tempo — three ways in, all landing on the same
                        // `set_tempo`: scroll (shift for ×10 steps, matches
                        // the original's `onTempoWheel`), the chevrons
                        // (guaranteed click path), and a vertical drag on the
                        // pill itself (continuous, and — unlike scroll —
                        // not dependent on this platform's wheel-event
                        // reporting). Double-click swaps the display for
                        // `AppShell::tempo_input` to type an exact value.
                        let editing = self.toolbar.editing_tempo;
                        let mut pill = div()
                            .id("tempo")
                            .flex()
                            .flex_col()
                            .items_center()
                            .px_2()
                            .py_0p5()
                            .rounded(theme::RADIUS_SM)
                            .bg(theme::SLATE)
                            .when(!editing, |d| d.cursor_ns_resize())
                            .hover(|s| s.bg(theme::BTN_HOVER))
                            .child(div().text_size(px(8.5)).text_color(theme::accent_dim()).child("BPM"));
                        pill = if editing {
                            pill.child(
                                // Generously wide, not sized to exactly fit
                                // "130.0" — `Input`'s actual rendered glyph
                                // size didn't reliably match the `.text_size()`
                                // asked for below, and a box sized to the
                                // smaller assumed size clipped the end of the
                                // number (only revealed by the caret's own
                                // keep-visible scroll when clicking into it).
                                // Erring wide costs nothing here; erring
                                // narrow silently eats digits. No fixed
                                // height either, for the same reason: forcing
                                // one shorter than the input's own natural
                                // line height pushed the text out below it
                                // instead of clipping it horizontally.
                                div().w(px(92.0)).child(
                                    Input::new(&self.tempo_input)
                                        .appearance(false)
                                        .text_size(px(14.0))
                                        .text_align(gpui::TextAlign::Center)
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(theme::accent()),
                                ),
                            )
                        } else {
                            pill.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(tempo_step_btn("tempo-dec", IconName::ChevronDown, -1.0, cx))
                                    .child(
                                        div()
                                            .min_w(px(36.0))
                                            .text_size(px(14.0))
                                            .text_align(gpui::TextAlign::Center)
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(theme::accent())
                                            .child(format!("{tempo:.1}")),
                                    )
                                    .child(tempo_step_btn("tempo-inc", IconName::ChevronUp, 1.0, cx)),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, ev: &MouseDownEvent, _, cx| this.begin_tempo_drag(ev.position.y, cx)),
                            )
                            .on_click(cx.listener(|this, ev: &ClickEvent, window, cx| {
                                if ev.click_count() >= 2 {
                                    this.start_edit_tempo(window, cx);
                                }
                            }))
                            .on_scroll(cx.listener(|this, ev: &ScrollEvent, _, cx| {
                                let raw = ev.delta.pixel_delta(px(1.0));
                                let dy = f32::from(raw.y);
                                let step = if ev.modifiers.shift { 10.0 } else { 1.0 };
                                let sign = if dy > 0.0 { -1.0 } else if dy < 0.0 { 1.0 } else { 0.0 };
                                let current = this.channel_rack.read(cx).tempo();
                                this.channel_rack.update(cx, |cr, cx| cr.set_tempo(current + sign * step, cx));
                            }))
                            .on_hover(cx.listener(move |shell, hovering: &bool, _, cx| {
                                if *hovering {
                                    shell.toolbar.hover = Some(("Tempo", format!("{tempo:.1} BPM")));
                                    cx.notify();
                                }
                            }))
                        };
                        pill
                    })
                    .child(vsep())
                    .child(
                        // Step position — this rewrite's grid is step-based rather
                        // than the original's beat:step:tick clock, so this shows
                        // the current step count instead of a full timecode.
                        div()
                            .id("step-pos")
                            .flex()
                            .flex_col()
                            .items_center()
                            .px_2()
                            .py_0p5()
                            .rounded(theme::RADIUS_SM)
                            .bg(theme::SLATE)
                            .child(div().text_size(px(8.5)).text_color(theme::accent_dim()).child("STEP"))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme::accent())
                                    .child(format!("{step:.0}")),
                            )
                            .on_hover(cx.listener(move |shell, hovering: &bool, _, cx| {
                                if *hovering {
                                    shell.toolbar.hover = Some(("Step position", format!("{step:.0}")));
                                    cx.notify();
                                }
                            })),
                    )
                    .child(vsep())
                    .child(
                        // Feature toggles: typing keyboard, precount, loop record, metronome.
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(toggle(
                                "feat-typing",
                                IconName::Keyboard,
                                self.toolbar.typing_keyboard,
                                theme::secondary(),
                                "Typing keyboard to piano",
                                if self.toolbar.typing_keyboard { "On" } else { "Off" },
                                |this, _, _, cx| {
                                    this.toolbar.typing_keyboard = !this.toolbar.typing_keyboard;
                                    cx.notify();
                                },
                                cx,
                            ))
                            .child(toggle(
                                "feat-precount",
                                IconName::Timer,
                                self.toolbar.precount,
                                theme::secondary(),
                                "Recording precount",
                                if self.toolbar.precount { "On" } else { "Off" },
                                |this, _, _, cx| {
                                    this.toolbar.precount = !this.toolbar.precount;
                                    cx.notify();
                                },
                                cx,
                            ))
                            .child(toggle(
                                "feat-looprec",
                                IconName::Repeat,
                                self.toolbar.loop_record,
                                theme::secondary(),
                                "Loop recording",
                                if self.toolbar.loop_record { "On" } else { "Off" },
                                |this, _, _, cx| {
                                    this.toolbar.loop_record = !this.toolbar.loop_record;
                                    cx.notify();
                                },
                                cx,
                            ))
                            .child(toggle(
                                "feat-metronome",
                                IconName::Triangle,
                                self.toolbar.metronome,
                                theme::secondary(),
                                "Metronome",
                                if self.toolbar.metronome { "On" } else { "Off" },
                                |this, _, _, cx| this.toggle_metronome(cx),
                                cx,
                            )),
                    )
                    .child(vsep())
                    .child(
                        div()
                            .id("monitor-scope")
                            .rounded(theme::RADIUS_XS)
                            .bg(theme::BTN_BG)
                            .p_1()
                            .cursor_pointer()
                            .child(monitor_view)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.toolbar.monitor_mode = match this.toolbar.monitor_mode {
                                    MonitorMode::Oscilloscope => MonitorMode::Spectrum,
                                    MonitorMode::Spectrum => MonitorMode::Oscilloscope,
                                };
                                cx.notify();
                            }))
                            .on_hover(cx.listener({
                                let label = monitor_system_label.clone();
                                move |shell, hovering: &bool, _, cx| {
                                    if *hovering {
                                        let (name, next) = match monitor_mode {
                                            MonitorMode::Oscilloscope => ("Oscilloscope", "spectrum analyzer"),
                                            MonitorMode::Spectrum => ("Spectrum analyzer", "oscilloscope"),
                                        };
                                        let value = match monitor_source {
                                            preferences::MonitorSource::Daw => format!("{name} — click for {next}"),
                                            preferences::MonitorSource::System => label.clone(),
                                        };
                                        shell.toolbar.hover = Some(("Audio monitor", value));
                                        cx.notify();
                                    }
                                }
                            })),
                    )
                    .child(
                        div()
                            .id("monitor-peak")
                            .child(peak_meter(peak_levels))
                            .on_hover(cx.listener(move |shell, hovering: &bool, _, cx| {
                                if *hovering {
                                    let value = match monitor_source {
                                        preferences::MonitorSource::Daw => {
                                            format!("L {:.1} dB · R {:.1} dB", lin_to_db(peak_levels.0), lin_to_db(peak_levels.1))
                                        }
                                        preferences::MonitorSource::System => monitor_system_label.clone(),
                                    };
                                    shell.toolbar.hover = Some(("Peak meter", value));
                                    cx.notify();
                                }
                            })),
                    )
                    .child(div().pr_2()),
            )
            .child(
                // ── Row 2: hover info, panel toggles, snap/pattern, MIDI, keys ──
                div()
                    .h(theme::TOOLBAR_ROW_H)
                    .w_full()
                    .flex()
                    .items_center()
                    .px_3()
                    .border_t_1()
                    .border_color(theme::BORDER)
                    .child(
                        div()
                            .w(px(200.0))
                            .flex()
                            .flex_col()
                            .justify_center()
                            .gap(px(1.0))
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::accent_dim())
                                    .child(hover.as_ref().map(|(name, _)| name.to_string()).unwrap_or_default()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::accent_dim().alpha(0.6))
                                    .child(hover.map(|(_, value)| value).unwrap_or_default()),
                            ),
                    )
                    .child(vsep())
                    .child(inert("panel-explorer", IconName::FolderOpen, "File Explorer", "Not built yet — MVP item #8", cx))
                    .child({
                        let mut pills = div().flex().gap_1();
                        for kind in PanelKind::ALL {
                            pills = pills.child(self.toggle_pill(kind, cx));
                        }
                        pills
                    })
                    .child(vsep())
                    .child(toggle(
                        "row2-autoscroll",
                        IconName::ChevronsRight,
                        self.toolbar.auto_scroll,
                        theme::secondary(),
                        "Auto scroll",
                        if self.toolbar.auto_scroll { "On" } else { "Off" },
                        |this, _, _, cx| {
                            this.toolbar.auto_scroll = !this.toolbar.auto_scroll;
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(toggle(
                        "row2-oneclickrec",
                        IconName::Mic,
                        self.toolbar.one_click_record,
                        theme::secondary(),
                        "One-click audio recording",
                        if self.toolbar.one_click_record { "On" } else { "Off" },
                        |this, _, _, cx| {
                            this.toolbar.one_click_record = !this.toolbar.one_click_record;
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(vsep())
                    .child(
                        ToggleGroup::new("snap")
                            .outline()
                            .segmented()
                            .single()
                            .small()
                            .child(Toggle::new("snap-1-4").label("1/4").checked(snap_quarter))
                            .child(Toggle::new("snap-1-2").label("1/2").checked(!snap_quarter))
                            .on_click(cx.listener(|this, checks: &Vec<bool>, _, cx| {
                                this.toolbar.snap_quarter = checks.first().copied().unwrap_or(true);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("row2-pattern")
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .h(px(26.0))
                            .rounded(theme::RADIUS_SM)
                            .border_1()
                            .border_color(theme::BTN_BORDER)
                            .bg(theme::BTN_BG)
                            .text_size(px(11.0))
                            .text_color(theme::BTN_TEXT)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::BTN_HOVER).text_color(theme::MAIN_TEXT))
                            .child(Icon::new(IconName::ListMusic).small())
                            .child(pattern_label.clone())
                            .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                                if patterns.is_empty() {
                                    return;
                                }
                                let pos = patterns.iter().position(|(id, _)| *id == selected_pattern_id).unwrap_or(0);
                                let next_id = patterns[(pos + 1) % patterns.len()].0;
                                this.channel_rack.update(cx, |cr, cx| cr.select_pattern(next_id, cx));
                            }))
                            .on_hover(cx.listener(move |shell, hovering: &bool, _, cx| {
                                if *hovering {
                                    shell.toolbar.hover = Some(("Pattern", pattern_label.clone()));
                                    cx.notify();
                                }
                            })),
                    )
                    .child(div().flex_1())
                    .child(inert("midi-led", IconName::Radio, "MIDI", "No MIDI I/O in this rewrite", cx))
                    .child(vsep())
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(key_badge("SHF", modifiers.shift))
                            .child(key_badge("ALT", modifiers.alt))
                            .child(key_badge("CTL", modifiers.control)),
                    ),
            )
    }
}

fn key_badge(label: &'static str, held: bool) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .rounded(theme::RADIUS_SM)
        .border_1()
        .text_size(px(9.0))
        .font_weight(gpui::FontWeight::BOLD)
        .bg(if held { theme::accent().alpha(0.14) } else { theme::BTN_BG })
        .border_color(if held { theme::accent() } else { theme::BTN_BORDER })
        .text_color(if held { theme::accent() } else { theme::BTN_TEXT.alpha(0.5) })
        .child(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic signal, not a live capture — unlike `pw-record`'s own
    /// round trip (checked interactively while building this, not as a
    /// committed test: a live loopback capture on this machine showed
    /// real low-frequency content *alongside* the test tone, which is a
    /// property of this machine's audio chain, not something either this
    /// function or that test could fix), this isolates `spectrum_
    /// magnitude`'s own math from anything the audio chain might do.
    #[test]
    fn spectrum_magnitude_peaks_near_the_right_bin() {
        let rate = 44_100.0f32;
        let freq = 440.0f32;
        let samples: Vec<f32> = (0..audio::FFT_LEN).map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / rate).sin()).collect();
        let mut planner = rustfft::FftPlanner::new();
        let fft = planner.plan_fft_forward(audio::FFT_LEN);
        let mags = spectrum_magnitude(&samples, fft.as_ref());

        let (peak_bin, _) = mags.iter().enumerate().fold((0, 0.0f32), |acc, (i, &m)| if m > acc.1 { (i, m) } else { acc });
        let bin_hz = rate / audio::FFT_LEN as f32;
        let peak_hz = peak_bin as f32 * bin_hz;
        assert!((400.0..480.0).contains(&peak_hz), "expected the peak near 440Hz, got {peak_hz:.0}Hz");
    }

    #[test]
    fn bin_edge_is_monotonic_and_spans_the_full_bin_range() {
        let bins = 512;
        let mut prev = 0;
        for bar in 0..SPECTRUM_BARS {
            let edge = bin_edge(bar, SPECTRUM_BARS, bins);
            assert!(edge >= prev, "bin_edge should never go backwards (bar {bar}: {edge} < {prev})");
            prev = edge;
        }
        assert_eq!(bin_edge(SPECTRUM_BARS, SPECTRUM_BARS, bins), bins - 1, "the last edge should reach the top of the bin range");
    }
}
