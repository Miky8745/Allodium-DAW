//! One channel row: mute/solo dot, pan/volume dials, mixer-track route,
//! sample box (audition/rename/recolor/delete live here), the row
//! select-toggle, and the 16-step sequencer. Ports `ChannelRow.svelte`.

use gpui::component::input::Input;
use gpui::component::menu::{ContextMenuExt, PopupMenuItem};
use gpui::{App, ClickEvent, Context, Div, Entity, Focusable, Hsla, IntoElement, MouseButton, MouseDownEvent, ParentElement, ScrollEvent, Styled, Window, div, prelude::*, px};

use crate::color_utils::{contrast_text_color, hsl_to_rgb_u32};
use crate::file_browser::SampleDragPayload;
use crate::project::{Note, STEP_PITCH};
use crate::theme;
use crate::widgets::{dial, scroll_field};

use super::drag::ParamKind;
use super::{ChannelRack, ChannelRackEvent};

/// Same per-entry-unique-id scheme as `piano_roll.rs`'s own `next_note_id`
/// — duplicated rather than shared, since each is private to its own
/// module and the two have no other reason to depend on each other.
fn next_note_id(notes: &[Note]) -> u32 {
    notes.iter().map(|n| n.id).max().unwrap_or(0) + 1
}

/// Whether a step-shaped note (the step sequencer's own domain: fixed
/// pitch, exactly one step long) sits at `step` — used both to light up
/// the step button and to find/remove it on toggle-off. Doesn't touch an
/// arbitrary piano-roll note of a different pitch/length that happens to
/// start at the same step; see `set_step`'s doc comment.
fn step_note_index(notes: &[Note], step: usize) -> Option<usize> {
    notes.iter().position(|n| n.pitch == STEP_PITCH && n.length == 1.0 && n.start as usize == step)
}

/// Blends two colors channel-by-channel, `t = 0` giving `from` and `t = 1`
/// giving `to` — used to fade a just-fired step's flash back down to its
/// resting color.
fn lerp_hsla(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    Hsla {
        h: from.h + (to.h - from.h) * t,
        s: from.s + (to.s - from.s) * t,
        l: from.l + (to.l - from.l) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

impl ChannelRack {
    /// Left/right mousedown on a step button set it on/off explicitly
    /// (rather than toggling) — Allodium-tauri's own `stepMousedown`: left
    /// always activates (and auditions), right always erases, so a quick
    /// left-right flick never leaves a step in the wrong state the way a
    /// plain toggle could. A step is just a `Note { pitch: STEP_PITCH,
    /// length: 1.0, .. }` — the step row and the Piano Roll are the same
    /// backend, so this only ever touches a note matching that exact
    /// shape, never an arbitrary note drawn in the Piano Roll that happens
    /// to start at the same step.
    fn set_step(&mut self, channel_id: u32, step: usize, active: bool, cx: &mut Context<Self>) {
        let entry = self.content_for(channel_id);
        let existing = step_note_index(&entry.notes, step);
        match (active, existing) {
            (true, None) => {
                let id = next_note_id(&entry.notes);
                entry.notes.push(Note { id, pitch: STEP_PITCH, start: step as f64, length: 1.0, velocity: 1.0 });
            }
            (false, Some(idx)) => {
                entry.notes.remove(idx);
            }
            _ => {}
        }
        if active {
            self.audition(channel_id);
        }
        // Grows the pattern when a step reaches its current last bar, or
        // shrinks it back down once trailing bars go empty again (e.g.
        // right-click-erasing the last note in one) — always exactly one
        // spare empty bar past the content, mirroring
        // `PianoRoll::commit_live`'s own call.
        self.sync_pattern_length(cx);
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    /// Fills every Nth step (0, N, 2N, …), clearing the rest — the row
    /// context menu's "Fill every N steps" (N = 2/4/8). Only ever adds/
    /// removes step-shaped notes (see `set_step`'s doc comment); any other
    /// note the Piano Roll drew for this channel is untouched. Never fills
    /// the pattern's own last bar — that's always `sync_pattern_length`'s
    /// spare empty bar, so a fill leaves it empty rather than immediately
    /// re-triggering its own growth.
    fn fill_every(&mut self, channel_id: u32, n: usize, cx: &mut Context<Self>) {
        let step_count = self.pattern_length_steps() as usize;
        let bar_steps = (self.beats_per_bar() as usize) * 4;
        let fill_count = step_count.saturating_sub(bar_steps);
        let entry = self.content_for(channel_id);
        entry.notes.retain(|note| !(note.pitch == STEP_PITCH && note.length == 1.0 && (note.start as usize) < step_count));
        let mut next_id = next_note_id(&entry.notes);
        for i in (0..fill_count).step_by(n) {
            entry.notes.push(Note { id: next_id, pitch: STEP_PITCH, start: i as f64, length: 1.0, velocity: 1.0 });
            next_id += 1;
        }
        self.sync_pattern_length(cx);
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    fn toggle_mute(&mut self, channel_id: u32, cx: &mut Context<Self>) {
        if let Some(ch) = self.project.channels.items.iter_mut().find(|c| c.id == channel_id) {
            ch.muted = !ch.muted;
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    /// Single-solo, matching the original: soloing one channel clears
    /// every other channel's solo instead of allowing several at once —
    /// except ctrl+clicking the mute button of the channel *already*
    /// soloed this way un-solos back to whatever was soloed right before,
    /// rather than just clearing everything (see `pre_solo_state`'s own
    /// doc comment).
    fn toggle_solo(&mut self, channel_id: u32, cx: &mut Context<Self>) {
        let was_soloed = self.project.channels.items.iter().find(|c| c.id == channel_id).map(|c| c.soloed).unwrap_or(false);
        if was_soloed {
            let restore = self.pre_solo_state.take().unwrap_or_default();
            for ch in &mut self.project.channels.items {
                ch.soloed = restore.contains(&ch.id);
            }
        } else {
            self.pre_solo_state = Some(self.project.channels.items.iter().filter(|c| c.soloed).map(|c| c.id).collect());
            for ch in &mut self.project.channels.items {
                ch.soloed = ch.id == channel_id;
            }
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    /// The row `.sel-btn` toggle's ctrl/shift-click range selection —
    /// ports `ChannelRack.svelte`'s `handleSelect` verbatim.
    fn select_row(&mut self, idx: usize, channel_id: u32, modifiers: gpui::Modifiers, cx: &mut Context<Self>) {
        if modifiers.control {
            if !self.multi_selected.remove(&channel_id) {
                self.multi_selected.insert(channel_id);
            }
        } else if modifiers.shift {
            if let Some(last) = self.last_select_idx {
                let ids: Vec<u32> = self.project.channels.items.iter().map(|c| c.id).collect();
                let (lo, hi) = (last.min(idx), last.max(idx));
                self.multi_selected = ids.get(lo..=hi).map(|s| s.iter().copied().collect()).unwrap_or_default();
            } else {
                self.multi_selected = std::iter::once(channel_id).collect();
            }
        } else {
            self.multi_selected = std::iter::once(channel_id).collect();
        }
        self.last_select_idx = Some(idx);
        cx.notify();
    }

    fn remove_channel(&mut self, channel_id: u32, cx: &mut Context<Self>) {
        self.project.channels.items.retain(|c| c.id != channel_id);
        self.project.content.items.retain(|e| e.channel_id != channel_id);
        self.multi_selected.remove(&channel_id);
        if self.selected_channel_id == Some(channel_id) {
            self.selected_channel_id = self.project.channels.items.first().map(|c| c.id);
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    fn start_rename(&mut self, channel_id: u32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ch) = self.project.channels.items.iter().find(|c| c.id == channel_id) else { return };
        let name = ch.name.clone();
        self.rename_input.update(cx, |s, cx| s.set_value(name, window, cx));
        self.renaming_channel_id = Some(channel_id);
        self.rename_input.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// Enter or losing focus commits — this view has no Escape-cancels-
    /// without-saving affordance the way the original's popup-menu-based
    /// rename did (`InputState`'s own `InputEvent` has no Escape/cancel
    /// variant to hook), so this is a deliberate simplification, not a
    /// missed port. Called from `ChannelRack::new`'s own subscription to
    /// `rename_input`, hence `pub(super)`.
    pub(super) fn commit_rename(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.renaming_channel_id.take() else { return };
        let value = self.rename_input.read(cx).value().trim().to_string();
        if let Some(ch) = self.project.channels.items.iter_mut().find(|c| c.id == id) {
            ch.name = value; // empty clears back to the auto-derived sample label
        }
        self.save();
        cx.notify();
    }

    /// "Recolor randomly" — ports `randomColorfulHex()` as a deterministic
    /// hue walk (see `recolor_seed`'s own doc comment) rather than pulling
    /// in a `rand` dependency for one context-menu item.
    fn recolor_randomly(&mut self, channel_id: u32, cx: &mut Context<Self>) {
        self.recolor_seed = self.recolor_seed.wrapping_add(1);
        const GOLDEN_RATIO_CONJUGATE: f32 = 0.618_034;
        let hue = ((channel_id as f32 + self.recolor_seed as f32 * 7.0) * GOLDEN_RATIO_CONJUGATE).fract();
        let rgb = hsl_to_rgb_u32(hue, 0.55, 0.55);
        if let Some(ch) = self.project.channels.items.iter_mut().find(|c| c.id == channel_id) {
            ch.color = rgb;
        }
        self.save();
        cx.notify();
    }

    /// Cycles a channel's mixer routing forward through Master → track 1 →
    /// track 2 → … → back to Master — the row's sample-adjacent "route"
    /// readout's click behavior. `cycle_channel_route_by` (scroll) is the
    /// closer port of the original's `ScrollField`; this wrapping version
    /// stays for a single click to still do something sensible.
    fn cycle_channel_route(&mut self, channel_id: u32, cx: &mut Context<Self>) {
        let mut targets: Vec<u32> = vec![0];
        targets.extend(self.project.mixer_tracks.items.iter().map(|t| t.id));
        if let Some(ch) = self.project.channels.items.iter_mut().find(|c| c.id == channel_id) {
            let pos = targets.iter().position(|t| *t == ch.mixer_track).unwrap_or(0);
            ch.mixer_track = targets[(pos + 1) % targets.len()];
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    /// Steps a channel's mixer routing by `delta` targets, clamped (not
    /// wrapped) at either end — ports `ChannelRow.svelte`'s mixer-track
    /// `ScrollField`'s own clamped-range scroll behavior.
    fn cycle_channel_route_by(&mut self, channel_id: u32, delta: i32, cx: &mut Context<Self>) {
        let mut targets: Vec<u32> = vec![0];
        targets.extend(self.project.mixer_tracks.items.iter().map(|t| t.id));
        if let Some(ch) = self.project.channels.items.iter_mut().find(|c| c.id == channel_id) {
            let pos = targets.iter().position(|t| *t == ch.mixer_track).unwrap_or(0) as i32;
            let new_pos = (pos + delta).clamp(0, targets.len() as i32 - 1);
            ch.mixer_track = targets[new_pos as usize];
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_row(
        &mut self,
        idx: usize,
        id: u32,
        _playing: bool,
        current_step: i64,
        fired_fade: f32,
        pattern_id: u32,
        name_col_width: f32,
        rack: &Entity<ChannelRack>,
        cx: &mut Context<Self>,
    ) -> Div {
        let ch = self.project.channels.items.iter().find(|c| c.id == id).unwrap().clone();
        let notes = self
            .project
            .content
            .items
            .iter()
            .find(|e| e.pattern_id == pattern_id && e.channel_id == id)
            .map(|e| e.notes.clone())
            .unwrap_or_default();

        let step_count = self.pattern_length_steps() as usize;
        let mut step_row = div().flex().flex_row().gap_1();
        for i in 0..step_count {
            let on = step_note_index(&notes, i).is_some();
            let is_current = current_step == i as i64;
            // 4-step beat-group banding, matching the Piano Roll's own
            // beat-group shading — the two grids should read as one
            // system, not two unrelated ones.
            let in_even_group = (i / 4) % 2 == 0;
            let bg = match (on, in_even_group) {
                (true, _) => theme::accent(),
                (false, true) => theme::STEP_OFF_GREY,
                (false, false) => theme::STEP_OFF_ORANGE,
            };
            step_row = step_row.child(
                div()
                    .id(("step", (id as usize) * 100 + i))
                    .w(px(20.0))
                    .h(px(28.0))
                    .rounded(theme::RADIUS_XS)
                    .bg(bg)
                    .when(is_current, |d| d.border_2().border_color(theme::accent_text()))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::BG_PILL_HOVER))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| this.set_step(id, i, true, cx)))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, _: &MouseDownEvent, _, cx| this.set_step(id, i, false, cx))),
            );
        }

        let name_label = if ch.name.is_empty() { "Drop sample".to_string() } else { ch.name.clone() };
        let is_renaming = self.renaming_channel_id == Some(id);
        let is_multi_selected = self.multi_selected.contains(&id);
        let is_piano_target = self.selected_channel_id == Some(id);

        let sample_box: gpui::AnyElement = if is_renaming {
            div()
                .id(("rename", id as usize))
                .w(px(name_col_width))
                .h(px(28.0))
                .rounded(theme::RADIUS_XS)
                .border_1()
                .border_color(theme::accent())
                .bg(theme::BG_APP.alpha(0.6))
                .flex()
                .items_center()
                .px_2()
                .child(Input::new(&self.rename_input).appearance(false).text_size(px(11.0)))
                .into_any_element()
        } else {
            let rack_open = rack.clone();
            let rack_fill2 = rack.clone();
            let rack_fill4 = rack.clone();
            let rack_fill8 = rack.clone();
            let rack_rename = rack.clone();
            let rack_recolor = rack.clone();
            let rack_delete = rack.clone();
            div()
                .id(("sample", id as usize))
                .w(px(name_col_width))
                .h(px(28.0))
                .rounded(theme::RADIUS_XS)
                .border_1()
                .border_color(theme::BORDER)
                .when(ch.color != 0, |d| d.bg(gpui::rgb(ch.color)))
                .when(ch.color == 0, |d| d.bg(theme::BG_APP.alpha(0.5)))
                .when(is_piano_target, |d| d.border_color(theme::accent()))
                .flex()
                .items_center()
                .px_2()
                .overflow_hidden()
                .cursor_default()
                .text_size(px(11.0))
                .text_color(if ch.color != 0 { contrast_text_color(ch.color) } else { theme::MAIN_TEXT })
                .child(name_label)
                // Dropping a sample dragged from the File Browser here
                // replaces this channel's sample in place — see
                // `ChannelRack::assign_sample`.
                .drag_over::<SampleDragPayload>(|style, _, _, _| style.border_color(theme::accent()).bg(theme::accent().alpha(0.18)))
                .on_drop(cx.listener(move |this, payload: &SampleDragPayload, _, cx| {
                    this.assign_sample(id, std::path::PathBuf::from(payload.path.clone()), cx);
                }))
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, _| this.audition(id)))
                .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                    if ev.click_count() >= 2 {
                        this.select_channel(id, cx);
                    }
                }))
                .context_menu(move |menu, _, _| {
                    menu.item(PopupMenuItem::new("Open in Piano Roll").on_click({
                        let rack = rack_open.clone();
                        move |_, _, cx: &mut App| rack.update(cx, |r, cx| r.select_channel(id, cx))
                    }))
                    .separator()
                    .item(PopupMenuItem::new("Fill every 2 steps").on_click({
                        let rack = rack_fill2.clone();
                        move |_, _, cx: &mut App| rack.update(cx, |r, cx| r.fill_every(id, 2, cx))
                    }))
                    .item(PopupMenuItem::new("Fill every 4 steps").on_click({
                        let rack = rack_fill4.clone();
                        move |_, _, cx: &mut App| rack.update(cx, |r, cx| r.fill_every(id, 4, cx))
                    }))
                    .item(PopupMenuItem::new("Fill every 8 steps").on_click({
                        let rack = rack_fill8.clone();
                        move |_, _, cx: &mut App| rack.update(cx, |r, cx| r.fill_every(id, 8, cx))
                    }))
                    .separator()
                    .item(PopupMenuItem::new("Rename").on_click({
                        let rack = rack_rename.clone();
                        move |_, window, cx: &mut App| rack.update(cx, |r, cx| r.start_rename(id, window, cx))
                    }))
                    .item(PopupMenuItem::new("Recolor randomly").on_click({
                        let rack = rack_recolor.clone();
                        move |_, _, cx: &mut App| rack.update(cx, |r, cx| r.recolor_randomly(id, cx))
                    }))
                    .separator()
                    .item(PopupMenuItem::new("Delete channel").on_click({
                        let rack = rack_delete.clone();
                        move |_, _, cx: &mut App| rack.update(cx, |r, cx| r.remove_channel(id, cx))
                    }))
                })
                .into_any_element()
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(theme::BORDER.alpha(0.4))
            .when(is_multi_selected, |d| d.bg(theme::accent().alpha(0.08)))
            .hover(|s| if is_multi_selected { s } else { s.bg(theme::BG_PILL.alpha(0.3)) })
            // Mute dot: plain click mutes; ctrl+click solos instead —
            // single-solo (`toggle_solo`), so soloing un-solos whichever
            // other channel was soloed. Ports `ChannelRow.svelte`'s
            // `toggleMuteOrSolo`.
            .child(
                div()
                    .id(("mute", id as usize))
                    .w(px(12.0))
                    .h(px(12.0))
                    .rounded_full()
                    .cursor_pointer()
                    .bg(if ch.soloed { theme::secondary() } else if ch.muted { theme::BTN_BORDER } else { theme::accent() })
                    .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        if ev.modifiers().control {
                            this.toggle_solo(id, cx);
                        } else {
                            this.toggle_mute(id, cx);
                        }
                    })),
            )
            .child(dial(
                ("pan", id as usize),
                ParamKind::Pan.name(),
                ParamKind::Pan.value_text(ch.pan),
                ch.pan,
                ParamKind::Pan.max(),
                ParamKind::Pan.sweep_start(),
                ParamKind::Pan.sweep_degrees(),
                24.0,
                cx.listener(move |this, ev: &MouseDownEvent, _, _| this.begin_param_drag(id, ParamKind::Pan, f32::from(ev.position.y))),
                {
                    let rack = rack.clone();
                    move |_: &mut Window, cx: &mut App| rack.update(cx, |r, cx| r.reset_param(id, ParamKind::Pan, cx))
                },
                cx.listener(move |_this, (hovering, name, value): &(bool, &'static str, String), _, cx| {
                    if *hovering {
                        cx.emit(ChannelRackEvent::HoverInfo(name, value.clone()));
                    }
                }),
            ))
            .child(dial(
                ("volume", id as usize),
                ParamKind::Volume.name(),
                ParamKind::Volume.value_text(ch.volume),
                ch.volume,
                ParamKind::Volume.max(),
                ParamKind::Volume.sweep_start(),
                ParamKind::Volume.sweep_degrees(),
                24.0,
                cx.listener(move |this, ev: &MouseDownEvent, _, _| this.begin_param_drag(id, ParamKind::Volume, f32::from(ev.position.y))),
                {
                    let rack = rack.clone();
                    move |_: &mut Window, cx: &mut App| rack.update(cx, |r, cx| r.reset_param(id, ParamKind::Volume, cx))
                },
                cx.listener(move |_this, (hovering, name, value): &(bool, &'static str, String), _, cx| {
                    if *hovering {
                        cx.emit(ChannelRackEvent::HoverInfo(name, value.clone()));
                    }
                }),
            ))
            .child(scroll_field(
                ("route", id as usize),
                if ch.mixer_track == 0 { "Mstr".to_string() } else { format!("Trk {}", ch.mixer_track) },
                40.0,
                cx.listener(move |this, ev: &ScrollEvent, _, cx| {
                    let dy = f32::from(ev.delta.pixel_delta(px(1.0)).y);
                    let sign: i32 = if dy > 0.0 { -1 } else if dy < 0.0 { 1 } else { 0 };
                    this.cycle_channel_route_by(id, sign, cx);
                }),
            ))
            .child(
                div()
                    .id(("route-click", id as usize))
                    .absolute() // overlays the scroll_field above for the click-cycle convenience; see cycle_channel_route's own doc comment
                    .size_full()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.cycle_channel_route(id, cx))),
            )
            .child(sample_box)
            .child(
                {
                    // Lights up when this channel fires a hit on the
                    // current step, then fades back down over
                    // `fired_fade`'s decay window — a per-channel "it just
                    // played" indicator, not a static selection box.
                    let channel_fired = current_step >= 0 && step_note_index(&notes, current_step as usize).is_some();
                    let flash = if channel_fired { fired_fade } else { 0.0 };
                    let resting_bg = if is_multi_selected { theme::accent() } else { gpui::transparent_black() };
                    let bg = if flash > 0.0 { lerp_hsla(resting_bg, theme::accent_hover(), flash) } else { resting_bg };
                    div()
                        .id(("select", id as usize))
                        .w(px(14.0))
                        .h(px(14.0))
                        .rounded(theme::RADIUS_XS)
                        .border_1()
                        .border_color(if is_multi_selected { theme::accent() } else { theme::BTN_BORDER })
                        .bg(bg)
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| this.select_row(idx, id, ev.modifiers(), cx)))
                },
            )
            .child(step_row)
    }
}
