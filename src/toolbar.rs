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
//! exist yet in this rewrite. Metronome goes one step further: it's backed
//! by a real audible click in the audio engine (`audio::AudioEngine::
//! set_metronome`, a synthesized accent/regular click scheduled once per
//! quarter note in the render callback) and ports the original's one real
//! side effect (turning it on auto-starts playback). The Explorer panel
//! toggle now opens the real File
//! Browser panel (`src/file_browser.rs`, MVP item #8) via the same
//! `AppShell::toggle_pill` the other floating panels use, just in its
//! own dedicated slot ahead of that generic pill cluster rather than
//! inside it. Only one control has no faithful equivalent at all and
//! stays inert, disabled chrome: the MIDI LED (no MIDI I/O anywhere in
//! this codebase).

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
use crate::floating_panel::{self, PanelKind, ShellDragMarker};
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
    /// Which of the three views the audio monitor shows — click cycles
    /// through all three (`Toolbar.svelte`'s own `BlackBox` behavior,
    /// extended with a third mode of its own: this app's XY/Lissajous
    /// oscilloscope has no original counterpart to port).
    monitor_mode: MonitorMode,
    /// `spectrogram`'s autoscale ceiling — the spectrum-analyzer
    /// equivalent of `wave_gain`, see `FREQ_RELEASE`/`FREQ_GAIN_FLOOR`.
    /// The actual FFT no longer runs here at all — see
    /// `audio::spawn_spectrum_worker`'s doc comment for why — so this is
    /// the only per-frame spectrum state left on the render path.
    freq_gain: f32,
    /// The XY oscilloscope's autoscale ceiling — same fast-attack/slow-
    /// release shape as `wave_gain`, but tracked separately and applied
    /// identically to both axes (never two independent gains), so the
    /// plotted shape scales but never distorts/stretches.
    xy_gain: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MonitorMode {
    Oscilloscope,
    Xy,
    Spectrum,
}

impl MonitorMode {
    /// Click-to-cycle order: oscilloscope -> XY -> spectrum -> back to
    /// oscilloscope.
    fn next(self) -> Self {
        match self {
            MonitorMode::Oscilloscope => MonitorMode::Xy,
            MonitorMode::Xy => MonitorMode::Spectrum,
            MonitorMode::Spectrum => MonitorMode::Oscilloscope,
        }
    }

    fn label(self) -> &'static str {
        match self {
            MonitorMode::Oscilloscope => "Oscilloscope",
            MonitorMode::Xy => "Oscilloscope (2D)",
            MonitorMode::Spectrum => "Spectrum analyzer",
        }
    }
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
            xy_gain: WAVE_GAIN_FLOOR,
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
    let value = value.into();
    // The base button (size, colors, hover) is `widgets::icon_toggle`;
    // this layers on the toolbar-specific glow and the hover-info wiring
    // (`shell.toolbar.hover`) that a context-agnostic shared widget can't
    // know about.
    crate::widgets::icon_toggle(id, icon, active, color, 28.0, 26.0, cx.listener(on_click))
        .when(active, |d| d.shadow(glow(color)))
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

/// A disabled toggle with no backend at all (just the MIDI LED now) —
/// its hover-info note says so honestly rather than implying it works.
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
    // A real traced waveform — Allodium-tauri's own `BlackBox.svelte`
    // `drawWave`'s `moveTo`/`lineTo` polyline through each sample's actual
    // signed deviation, not an abs-amplitude bar chart (which reads as a
    // level meter, not a waveform). The first version here used `canvas` +
    // `PathBuilder::stroke` for a literal connected line, which cost
    // ~100ms/frame on this machine's GPU backend once it painted every
    // frame — tanked the whole app from ~35fps to ~7fps (see the
    // following commit's follow-up for the fix that replaced it with
    // plain bars, which fixed the framerate but lost the actual waveform
    // shape in the process). This instead "connects the dots" with plain
    // `div`s: one thin absolutely-positioned bar per step spanning from
    // one sample's y to the next's, so consecutive segments' edges align
    // into a continuous (staircase, not diagonal, but visually
    // indistinguishable at this pixel density) trace — same per-div cost
    // the old bar chart (and the playlist's per-frame block layer) already
    // paid, with no canvas/path involved.
    //
    // Plots every sample in `samples` (no display-side subsampling) —
    // an earlier version here picked only 64 evenly-spaced samples out
    // of the buffer's full length, which is itself a ~16x decimation
    // (matching, coincidentally, the exact decimation factor a since-
    // removed audio-thread-side buffer used to apply) and showed up as
    // a visibly "bit-crushed" jagged trace on a user-reported pure sine
    // tone. `samples` is `ChannelRack::{,system_}scope_snapshot`'s raw,
    // non-decimated window (`audio::FFT_LEN` long, ~23ms) — the same
    // buffer the spectrum analyzer FFTs — so every segment here reflects
    // an actual consecutive sample pair, matching the original's canvas
    // trace (which maps its own full, non-decimated `fftSize` window
    // across the canvas the same way).
    const WIDTH: f32 = 90.0;
    const HEIGHT: f32 = 30.0;
    let n = samples.len().max(1);
    let y_at = |v: f32| HEIGHT / 2.0 - v.clamp(-1.0, 1.0) * (HEIGHT / 2.0 - 1.0);
    let dx = WIDTH / (n - 1).max(1) as f32;
    div()
        .relative()
        .w(px(WIDTH))
        .h(px(HEIGHT))
        .children((0..n.saturating_sub(1)).map(move |i| {
            let (y0, y1) = (y_at(samples[i]), y_at(samples[i + 1]));
            let top = y0.min(y1);
            let h = (y0 - y1).abs().max(1.0);
            div().absolute().left(px(i as f32 * dx)).top(px(top)).w(px(dx.max(1.0))).h(px(h)).bg(theme::accent())
        }))
}

/// The XY ("2D") oscilloscope — plots `(l[i], r[i])` as a point per
/// frame instead of amplitude-over-time, the way an actual X-Y
/// oscilloscope (two channels driving its horizontal/vertical deflection
/// instead of one channel driving a time sweep) reads a stereo signal.
/// This is what makes "oscilloscope music" — tracks specifically
/// authored so their two channels trace a recognizable shape on a
/// scope — show up correctly instead of as two independent, individually
/// meaningless waveforms. No original counterpart in Allodium-tauri to
/// port; this mode doesn't exist there.
///
/// Same reasoning as `oscilloscope` for *how* it's drawn (many small
/// `div`s, no canvas/path — that's the thing that cost ~100ms/frame on
/// this GPU backend), but connecting consecutive points with straight
/// segments the way `oscilloscope` does doesn't translate: there, `x`
/// only ever increases with `i`, so a vertical connector bar always
/// reached the next point. Here both axes move independently per step,
/// which a plain axis-aligned `div` can't trace diagonally. Instead this
/// plots every sample as its own small dot — dense enough (one per
/// `audio::FFT_LEN` sample, same ~23ms window `oscilloscope` uses) that
/// consecutive dots overlap and the path reads as continuous, the same
/// way a real CRT's persistence blurs a fast-moving beam's individual
/// positions into a solid trace.
fn xy_oscilloscope(l: &[f32], r: &[f32]) -> impl IntoElement {
    const WIDTH: f32 = 90.0;
    const HEIGHT: f32 = 30.0;
    const DOT: f32 = 2.0;
    // One shared scale for both axes (the caller's `xy_gain` already
    // normalized `l`/`r` to -1..1 using one ceiling) — using `HEIGHT`,
    // the smaller of the two dimensions, so the plotted shape never
    // exceeds the box vertically and stays circular rather than
    // stretched into an ellipse.
    let scale = HEIGHT / 2.0 - DOT;
    let n = l.len().min(r.len());
    div()
        .relative()
        .w(px(WIDTH))
        .h(px(HEIGHT))
        .children((0..n).map(move |i| {
            let x = WIDTH / 2.0 + l[i].clamp(-1.0, 1.0) * scale;
            let y = HEIGHT / 2.0 - r[i].clamp(-1.0, 1.0) * scale;
            div().absolute().left(px(x - DOT / 2.0)).top(px(y - DOT / 2.0)).w(px(DOT)).h(px(DOT)).rounded(px(1.0)).bg(theme::accent())
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

/// The spectrum-analyzer view — a continuous filled curve across
/// frequency, not `audio::SPECTRUM_BARS`' worth of visibly separate
/// rectangles. `bins` arrives already exponentially bucketed into
/// `SPECTRUM_BARS` real FFT-derived values by the background spectrum
/// worker (that bucketing stays — it's what gives low frequencies more
/// than a pixel or two of width); this just resamples those `SPECTRUM_
/// BARS` values up to `COLUMNS` thin bottom-anchored bars via linear
/// interpolation between neighboring bins, the same "more, thinner
/// segments read as one continuous shape instead of blocks" trick
/// `oscilloscope` uses for the waveform trace, applied here as a filled
/// area instead of a centered line.
fn spectrogram(bins: &[f32]) -> impl IntoElement {
    const WIDTH: f32 = 90.0;
    const HEIGHT: f32 = 30.0;
    const COLUMNS: usize = 90;
    let n = bins.len().max(1);
    let height_at = move |column: usize| {
        let pos = column as f32 / (COLUMNS - 1).max(1) as f32 * (n - 1) as f32;
        let i0 = pos.floor() as usize;
        let i1 = (i0 + 1).min(n - 1);
        let t = pos - i0 as f32;
        let v0 = bins.get(i0).copied().unwrap_or(0.0);
        let v1 = bins.get(i1).copied().unwrap_or(0.0);
        ((v0 + (v1 - v0) * t).clamp(0.0, 1.0) * (HEIGHT - 2.0)).max(2.0)
    };
    let dx = WIDTH / COLUMNS as f32;
    div()
        .relative()
        .w(px(WIDTH))
        .h(px(HEIGHT))
        .children((0..COLUMNS).map(move |i| {
            div().absolute().left(px(i as f32 * dx)).bottom_0().w(px(dx.max(1.0))).h(px(height_at(i))).bg(theme::accent())
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

    /// Ports the original's one real side effect of flipping this switch:
    /// turning the metronome on auto-starts playback if it isn't already
    /// running, so enabling it is heard immediately instead of silently
    /// arming; turning it back off does not stop playback.
    fn toggle_metronome(&mut self, cx: &mut Context<Self>) {
        self.toolbar.metronome = !self.toolbar.metronome;
        self.channel_rack.update(cx, |cr, _| cr.set_metronome(self.toolbar.metronome));
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
            self.toolbar.xy_gain = WAVE_GAIN_FLOOR;
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
            MonitorMode::Xy => {
                let (raw_l, raw_r) = match monitor_source {
                    preferences::MonitorSource::Daw => cr.xy_snapshot(),
                    preferences::MonitorSource::System => cr.system_xy_snapshot(),
                };
                // One shared ceiling for both axes — see `xy_gain`'s doc
                // comment: scaling L and R independently would stretch
                // the plotted shape instead of just resizing it.
                let frame_gain = raw_l.iter().chain(raw_r.iter()).fold(0.0f32, |m, v| m.max(v.abs()));
                self.toolbar.xy_gain = if frame_gain > self.toolbar.xy_gain { frame_gain } else { self.toolbar.xy_gain * WAVE_RELEASE };
                let xy_scale = 1.0 / self.toolbar.xy_gain.max(WAVE_GAIN_FLOOR);
                let l: Vec<f32> = raw_l.iter().map(|s| (s * xy_scale).clamp(-1.0, 1.0)).collect();
                let r: Vec<f32> = raw_r.iter().map(|s| (s * xy_scale).clamp(-1.0, 1.0)).collect();
                xy_oscilloscope(&l, &r).into_any_element()
            }
            MonitorMode::Spectrum => {
                // The FFT itself runs on its own background thread now
                // (`audio::spawn_spectrum_worker`), at a fixed cadence
                // independent of render rate — this just reads the
                // already-binned result (`SPECTRUM_BARS` atomic loads)
                // and applies the same autoscale envelope as the
                // oscilloscope's `wave_gain`.
                let raw_bins = match monitor_source {
                    preferences::MonitorSource::Daw => cr.spectrum_bins(),
                    preferences::MonitorSource::System => cr.system_spectrum_bins(),
                };
                let frame_gain = raw_bins.iter().cloned().fold(0.0f32, f32::max);
                self.toolbar.freq_gain = if frame_gain > self.toolbar.freq_gain { frame_gain } else { self.toolbar.freq_gain * FREQ_RELEASE };
                let fgain = self.toolbar.freq_gain.max(FREQ_GAIN_FLOOR);
                let scaled: Vec<f32> = raw_bins.iter().map(|m| (m / fgain).clamp(0.0, 1.0)).collect();
                spectrogram(&scaled).into_any_element()
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
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| this.cancel_pending_click(cx)),
                            )
                            .on_drag(ShellDragMarker, floating_panel::drag_preview)
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
                                this.toolbar.monitor_mode = this.toolbar.monitor_mode.next();
                                cx.notify();
                            }))
                            .on_hover(cx.listener({
                                let label = monitor_system_label.clone();
                                move |shell, hovering: &bool, _, cx| {
                                    if *hovering {
                                        let (name, next) = (monitor_mode.label(), monitor_mode.next().label());
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
                    .child(self.file_browser_toggle_pill(cx))
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

