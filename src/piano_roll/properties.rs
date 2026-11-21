//! The Note Properties sidebar: velocity/pan/release/filter/pitch-bend
//! dials, the color swatch picker, and the slide toggle — Allodium-
//! tauri's own `NotePropertiesPanel.svelte`. See `piano_roll.rs`'s own
//! top doc comment for how far this goes beyond the original MVP scope
//! and what's still simplified.

use gpui::assets::IconName;
use gpui::component::{Icon, Sizable};
use gpui::{Context, IntoElement, MouseDownEvent, ParentElement, Styled, Window, div, prelude::*, px};

use crate::project::Note;
use crate::theme;

use super::PianoRoll;

const DEFAULT_VELOCITY: f64 = 1.0;

/// Every per-note dial in `render_note_properties` edits one of these —
/// shared getter/setter/display logic instead of six near-identical
/// dials, same reasoning `channel_rack::drag::ParamKind` already uses for
/// Pan/Volume. Velocity aside (already an MVP field), these are all the
/// "note properties beyond velocity" `piano_roll.rs`'s own top doc
/// comment used to list as declined.
#[derive(Clone, Copy, PartialEq)]
enum NoteParam {
    Velocity,
    /// Offset on top of the channel's own pan — `Note::pan`.
    Pan,
    /// `Note::release` — how far the release tail rings past the note's
    /// own scheduled end.
    Release,
    /// Filter cutoff — `Note::mod_x`.
    Cutoff,
    /// Filter resonance/Q — `Note::mod_y`.
    Resonance,
    /// Fine pitch in cents ("pitch bend") — `Note::fine_pitch`.
    FinePitch,
}

impl NoteParam {
    const ALL: [NoteParam; 6] =
        [NoteParam::Velocity, NoteParam::Pan, NoteParam::Release, NoteParam::Cutoff, NoteParam::Resonance, NoteParam::FinePitch];

    fn name(self) -> &'static str {
        match self {
            NoteParam::Velocity => "Velocity",
            NoteParam::Pan => "Pan",
            NoteParam::Release => "Release",
            NoteParam::Cutoff => "Cutoff",
            NoteParam::Resonance => "Resonance",
            NoteParam::FinePitch => "Pitch bend",
        }
    }

    fn min(self) -> f64 {
        if self == NoteParam::FinePitch { -1200.0 } else { 0.0 }
    }

    fn max(self) -> f64 {
        if self == NoteParam::FinePitch { 1200.0 } else { 1.0 }
    }

    /// The value a dial resets to on double-click/"Reset" — distinct
    /// from `min()` since several of these (pan, release, cutoff) have a
    /// "neutral/no effect" value in the middle or at the top of their
    /// range, not at zero.
    fn reset_value(self) -> f64 {
        match self {
            NoteParam::Velocity => 1.0,
            NoteParam::Pan => 0.5,
            NoteParam::Release => 0.5,
            NoteParam::Cutoff => 1.0,
            NoteParam::Resonance => 0.0,
            NoteParam::FinePitch => 0.0,
        }
    }

    fn get(self, note: &Note) -> f64 {
        match self {
            NoteParam::Velocity => note.velocity,
            NoteParam::Pan => note.pan,
            NoteParam::Release => note.release,
            NoteParam::Cutoff => note.mod_x,
            NoteParam::Resonance => note.mod_y,
            NoteParam::FinePitch => note.fine_pitch,
        }
    }

    fn set(self, note: &mut Note, value: f64) {
        match self {
            NoteParam::Velocity => note.velocity = value,
            NoteParam::Pan => note.pan = value,
            NoteParam::Release => note.release = value,
            NoteParam::Cutoff => note.mod_x = value,
            NoteParam::Resonance => note.mod_y = value,
            NoteParam::FinePitch => note.fine_pitch = value,
        }
    }

    fn get_default(self, defaults: &NoteDefaults) -> f64 {
        match self {
            NoteParam::Velocity => defaults.velocity,
            NoteParam::Pan => defaults.pan,
            NoteParam::Release => defaults.release,
            NoteParam::Cutoff => defaults.mod_x,
            NoteParam::Resonance => defaults.mod_y,
            NoteParam::FinePitch => defaults.fine_pitch,
        }
    }

    fn set_default(self, defaults: &mut NoteDefaults, value: f64) {
        match self {
            NoteParam::Velocity => defaults.velocity = value,
            NoteParam::Pan => defaults.pan = value,
            NoteParam::Release => defaults.release = value,
            NoteParam::Cutoff => defaults.mod_x = value,
            NoteParam::Resonance => defaults.mod_y = value,
            NoteParam::FinePitch => defaults.fine_pitch = value,
        }
    }

    /// Mirrors the audio engine's own mapping for cutoff/release
    /// (`audio::mod_x_to_cutoff_hz`/`audio::envelope`'s `release_seconds`)
    /// so the dial's readout always matches what's actually audible —
    /// duplicated rather than shared across the UI/engine boundary, same
    /// trade `widgets::dial`'s own callers already make for their value
    /// text.
    fn value_text(self, value: f64) -> String {
        match self {
            NoteParam::Velocity | NoteParam::Resonance => format!("{:.0}%", value * 100.0),
            NoteParam::Pan => format!("{:+.2}", value - 0.5),
            NoteParam::Release => {
                let r = value.clamp(0.0, 1.0);
                let secs = if r <= 0.5 { (r / 0.5) * 0.05 } else { 0.05 + ((r - 0.5) / 0.5) * (2.0 - 0.05) };
                format!("{:.2}s", secs)
            }
            NoteParam::Cutoff => {
                let hz = 50.0 * (20_000.0_f64 / 50.0).powf(value.clamp(0.0, 1.0));
                format!("{:.0}Hz", hz)
            }
            NoteParam::FinePitch => format!("{:+.0}c", value),
        }
    }
}

/// What a freshly placed note's properties start at, and what the
/// properties panel edits when nothing is selected — Allodium-tauri's own
/// `noteDefaults`.
#[derive(Clone, Copy)]
pub(super) struct NoteDefaults {
    pub(super) velocity: f64,
    pub(super) pan: f64,
    pub(super) release: f64,
    pub(super) mod_x: f64,
    pub(super) mod_y: f64,
    pub(super) fine_pitch: f64,
    pub(super) color: u32,
    pub(super) slide: bool,
}

impl Default for NoteDefaults {
    fn default() -> Self {
        let n = Note::default();
        NoteDefaults { velocity: DEFAULT_VELOCITY, pan: n.pan, release: n.release, mod_x: n.mod_x, mod_y: n.mod_y, fine_pitch: n.fine_pitch, color: n.color, slide: n.slide }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ParamDrag {
    param: NoteParam,
    start_mouse_y: f32,
    start_value: f64,
}

/// A fixed palette of evenly-spaced hues for the properties panel's color
/// swatches — Allodium-tauri's own fixed 15-color note palette, just
/// generated instead of hand-picked (same `hsl_to_rgb_u32` the Channel
/// Rack's "Recolor randomly" already uses).
fn note_color_palette() -> Vec<u32> {
    const COUNT: usize = 10;
    (0..COUNT).map(|i| crate::color_utils::hsl_to_rgb_u32(i as f32 / COUNT as f32, 0.55, 0.55)).collect()
}

impl PianoRoll {
    /// Note Properties panel: velocity, pan, release, filter cutoff/
    /// resonance, and fine pitch ("pitch bend") dials (`NoteParam`), plus
    /// a color swatch picker and a slide toggle. Edits every selected
    /// note at once when there's a selection; otherwise edits
    /// `self.defaults` (what a freshly placed note gets next), matching
    /// the original's own "edits the selected notes when there are any,
    /// otherwise `noteDefaults`." Mixed-value display (the original's
    /// dimmed "MIXED" label when selected notes disagree) is skipped —
    /// this just shows the first selected note's value for each control.
    /// Docked on the Piano Roll's left, same as Allodium-tauri's own
    /// `.np` (`NotePropertiesPanel.svelte`) — a fixed-width vertical
    /// column (scope label, dials in a 3-wide grid, slide toggle, color
    /// palette), not a header row, placed as the first child of the
    /// piano-keys+grid flex row in `render` rather than above the whole
    /// panel.
    pub(super) fn render_note_properties(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const PANEL_W: f32 = 138.0;

        let first_selected = self.selected.iter().next().and_then(|id| self.notes.iter().find(|n| n.id == *id));
        let label = if self.selected.is_empty() {
            "New notes".to_string()
        } else {
            format!("{} selected note{}", self.selected.len(), if self.selected.len() == 1 { "" } else { "s" })
        };
        let selected_color = first_selected.map(|n| n.color).unwrap_or(self.defaults.color);
        let selected_slide = first_selected.map(|n| n.slide).unwrap_or(self.defaults.slide);
        let this_entity = cx.entity();

        let dial_cell = |this: &Self, cx: &mut Context<Self>, param: NoteParam| {
            let value = first_selected.map(|n| param.get(n)).unwrap_or_else(|| param.get_default(&this.defaults));
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(crate::widgets::dial(
                    ("pr-param", param as usize),
                    param.name(),
                    param.value_text(value),
                    value - param.min(),
                    param.max() - param.min(),
                    210.0,
                    300.0,
                    20.0,
                    cx.listener(move |this, ev: &MouseDownEvent, _, _| this.begin_param_drag(param, f32::from(ev.position.y))),
                    {
                        let entity = this_entity.clone();
                        move |_: &mut Window, cx: &mut gpui::App| entity.update(cx, |pr, cx| pr.set_param(param, param.reset_value(), cx))
                    },
                    cx.listener(|_this, _ev: &(bool, &'static str, String), _, _| {}),
                ))
                .child(div().text_size(px(8.0)).text_color(theme::BTN_TEXT).child(param.name()))
        };

        let mut dial_rows = div().flex().flex_col().gap_2();
        for chunk in NoteParam::ALL.chunks(3) {
            let mut dial_row = div().flex().justify_between();
            for param in chunk {
                dial_row = dial_row.child(dial_cell(self, cx, *param));
            }
            dial_rows = dial_rows.child(dial_row);
        }

        div()
            .id("note-properties")
            .w(px(PANEL_W))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_3()
            .p_2()
            .overflow_y_scroll()
            .bg(theme::HEADER_BG)
            .border_r_1()
            .border_color(theme::BORDER)
            .child(
                div()
                    .text_size(px(10.0))
                    .text_center()
                    .text_color(if self.selected.is_empty() { theme::BTN_TEXT } else { theme::accent() })
                    .child(label),
            )
            .child(dial_rows)
            .child(
                div()
                    .id("pr-slide")
                    .w_full()
                    .h(px(26.0))
                    .rounded(theme::RADIUS_SM)
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .cursor_pointer()
                    .text_size(px(11.0))
                    .border_1()
                    .text_color(if selected_slide { theme::accent() } else { theme::BTN_TEXT })
                    .border_color(if selected_slide { theme::accent().alpha(0.4) } else { theme::BTN_BORDER })
                    .when(selected_slide, |d| d.bg(theme::accent().alpha(0.1)))
                    .hover(|s| if selected_slide { s } else { s.bg(theme::BTN_HOVER).text_color(theme::MAIN_TEXT) })
                    .child(Icon::new(IconName::TrendingUp).small())
                    .child("Slide")
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| this.set_slide(!selected_slide, cx))),
            )
            .child({
                let swatches = note_color_palette().into_iter().enumerate().map(|(i, rgb)| {
                    let active = selected_color == rgb;
                    div()
                        .id(("pr-color", i))
                        .w(px(16.0))
                        .h(px(16.0))
                        .rounded(theme::RADIUS_XS)
                        .bg(gpui::rgb(rgb))
                        .cursor_pointer()
                        .when(active, |d| d.border_2().border_color(theme::MAIN_TEXT))
                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| this.set_color(rgb, cx)))
                        .into_any_element()
                });
                let none_swatch = {
                    let active = selected_color == 0;
                    div()
                        .id("pr-color-none")
                        .w(px(16.0))
                        .h(px(16.0))
                        .rounded(theme::RADIUS_XS)
                        .border_1()
                        .border_color(theme::BTN_BORDER)
                        .cursor_pointer()
                        .when(active, |d| d.border_2().border_color(theme::MAIN_TEXT))
                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| this.set_color(0, cx)))
                        .into_any_element()
                };
                div().flex().flex_wrap().gap_1().justify_center().children(swatches.chain(std::iter::once(none_swatch)))
            })
    }

    fn begin_param_drag(&mut self, param: NoteParam, start_mouse_y: f32) {
        let start_value =
            self.selected.iter().next().and_then(|id| self.notes.iter().find(|n| n.id == *id)).map(|n| param.get(n)).unwrap_or_else(|| param.get_default(&self.defaults));
        self.param_drag = Some(ParamDrag { param, start_mouse_y, start_value });
    }

    pub(super) fn update_param_drag(&mut self, mouse_y: f32, cx: &mut Context<Self>) {
        let Some(drag) = self.param_drag else { return };
        const PARAM_DRAG_SENSITIVITY: f32 = 150.0;
        let dy = drag.start_mouse_y - mouse_y;
        let span = drag.param.max() - drag.param.min();
        let value = (drag.start_value + (dy / PARAM_DRAG_SENSITIVITY) as f64 * span).clamp(drag.param.min(), drag.param.max());
        self.set_param(drag.param, value, cx);
    }

    pub(super) fn end_param_drag(&mut self) {
        self.param_drag = None;
    }

    /// Writes `value` to every selected note's `param`, or to
    /// `self.defaults` (what the *next* placed note gets) with nothing
    /// selected — every properties-panel dial drives this, same split as
    /// the original's "edits the selected notes when there are any,
    /// otherwise `noteDefaults`."
    fn set_param(&mut self, param: NoteParam, value: f64, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            param.set_default(&mut self.defaults, value);
            cx.notify();
            return;
        }
        for note in self.notes.iter_mut().filter(|n| self.selected.contains(&n.id)) {
            param.set(note, value);
        }
        let Some(channel_id) = self.loaded_channel_id else { return };
        self.commit_live(channel_id, cx);
    }

    fn set_color(&mut self, rgb: u32, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.defaults.color = rgb;
            cx.notify();
            return;
        }
        for note in self.notes.iter_mut().filter(|n| self.selected.contains(&n.id)) {
            note.color = rgb;
        }
        let Some(channel_id) = self.loaded_channel_id else { return };
        self.commit_live(channel_id, cx);
    }

    fn set_slide(&mut self, slide: bool, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.defaults.slide = slide;
            cx.notify();
            return;
        }
        for note in self.notes.iter_mut().filter(|n| self.selected.contains(&n.id)) {
            note.slide = slide;
        }
        let Some(channel_id) = self.loaded_channel_id else { return };
        self.commit_live(channel_id, cx);
    }
}
