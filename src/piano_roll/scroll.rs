//! Horizontal navigation for a pattern wider than the panel: content/
//! viewport sizing, the draggable scrollbar below the grid, the bar-
//! numbered ruler above it, and the overview/minimap strip above that.
//! See `piano_roll.rs`'s own top doc comment for why these exist.

use gpui::{Context, IntoElement, MouseButton, MouseDownEvent, ParentElement, Pixels, Point, Styled, div, prelude::*, px};

use crate::theme;

use super::{PianoRoll, PITCH_COUNT, STEP_W, step_to_x};

/// Height of the overview strip (`render_overview`), shared with
/// `piano_roll.rs`'s own header-spacer above the piano-keys sidebar — see
/// that spacer's doc comment for why it has to match this exactly.
pub(super) const OVERVIEW_H: f32 = 26.0;
/// Height of the bar-numbered ruler (`render_beat_ruler`), same sharing
/// reason as `OVERVIEW_H` above.
pub(super) const RULER_H: f32 = 18.0;

/// Drag state for the horizontal scrollbar's thumb (`render_h_scrollbar`)
/// — tracked at the root level the same way `properties::ParamDrag` is,
/// since a drag has to keep updating even once the cursor leaves the
/// thumb's own (often narrow) hitbox.
#[derive(Clone, Copy)]
pub(super) struct HScrollDrag {
    start_mouse_x: f32,
    start_scroll_x: f32,
}

/// The shared "where is the viewport within the content, in pixels"
/// facts both `render_h_scrollbar` and `render_overview` draw from —
/// `PianoRoll::h_scroll_geometry`'s own doc comment.
#[derive(Clone, Copy)]
struct HScrollGeometry {
    viewport_w: f32,
    content_w: f32,
    max_scroll: f32,
    thumb_w: f32,
    track_free: f32,
    thumb_x: f32,
}

/// The horizontal scrollbar's thumb width — proportional to how much of
/// `content_w` is visible, same ratio a native scrollbar uses, clamped to
/// a minimum so it never shrinks to an undraggable sliver on a very long
/// pattern.
fn h_scrollbar_thumb_width(viewport_w: f32, content_w: f32) -> f32 {
    const MIN_THUMB_W: f32 = 24.0;
    // `viewport_w` starts at `0.0` before the grid's first paint
    // (`grid_size`'s own doc comment) and can legitimately be narrower
    // than `MIN_THUMB_W` in a small window — `.clamp(MIN_THUMB_W,
    // viewport_w)` panics the instant `viewport_w < MIN_THUMB_W` (min >
    // max), so clamp the upper bound to never go below the lower one
    // instead of assuming the panel is always wide enough.
    let max = viewport_w.max(MIN_THUMB_W);
    (viewport_w / content_w.max(1.0) * viewport_w).clamp(MIN_THUMB_W, max)
}

impl PianoRoll {
    pub(super) fn content_width(&self) -> f32 {
        self.pattern_length_steps as f32 * STEP_W
    }

    pub(super) fn content_height(&self) -> f32 {
        PITCH_COUNT as f32 * super::KEY_H
    }

    /// Clamps scroll so the viewport never scrolls past the end of the
    /// content (max scroll = content size minus the visible viewport size).
    pub(super) fn clamp_scroll(&mut self, viewport: gpui::Size<gpui::Pixels>) {
        let max_x = (self.content_width() - f32::from(viewport.width)).max(0.0);
        let max_y = (self.content_height() - f32::from(viewport.height)).max(0.0);
        self.scroll_x = self.scroll_x.clamp(0.0, max_x);
        self.scroll_y = self.scroll_y.clamp(0.0, max_y);
    }

    /// Everything the horizontal scrollbar (`render_h_scrollbar`) *and*
    /// the overview strip's viewport window (`render_overview`) need to
    /// draw/drag themselves — both are really the same "where is the
    /// current viewport within the content" fact, just drawn in two
    /// different places, so they share one computation instead of each
    /// re-deriving it (and risking disagreeing with each other).
    fn h_scroll_geometry(&self) -> HScrollGeometry {
        let viewport_w = f32::from(self.grid_size.get().width).max(0.0);
        let content_w = self.content_width();
        let max_scroll = (content_w - viewport_w).max(0.0);
        let thumb_w = h_scrollbar_thumb_width(viewport_w, content_w);
        let track_free = (viewport_w - thumb_w).max(1.0);
        let thumb_x = if max_scroll > 0.0 { (self.scroll_x / max_scroll) * track_free } else { 0.0 };
        HScrollGeometry { viewport_w, content_w, max_scroll, thumb_w, track_free, thumb_x }
    }

    fn begin_h_scroll_drag(&mut self, start_mouse_x: f32) {
        self.h_scroll_drag = Some(HScrollDrag { start_mouse_x, start_scroll_x: self.scroll_x });
    }

    /// Converts a thumb drag's on-screen pixel delta into a `scroll_x`
    /// delta by the track/content ratio — the thumb only ever travels
    /// `track_free` px while the content scrolls the much larger
    /// `max_scroll` px, so a 1:1 pixel mapping would undershoot badly on
    /// a long pattern.
    pub(super) fn update_h_scroll_drag(&mut self, mouse_x: f32, cx: &mut Context<Self>) {
        let Some(drag) = self.h_scroll_drag else { return };
        let geo = self.h_scroll_geometry();
        let dx = mouse_x - drag.start_mouse_x;
        self.scroll_x = (drag.start_scroll_x + dx * (geo.max_scroll / geo.track_free)).clamp(0.0, geo.max_scroll);
        cx.notify();
    }

    pub(super) fn end_h_scroll_drag(&mut self) {
        self.h_scroll_drag = None;
    }

    /// Clicking the scrollbar track or the overview strip's background
    /// (not the thumb/window itself — those stop propagation before this
    /// ever fires) jumps so the thumb/window's center lands under the
    /// cursor.
    fn jump_h_scroll(&mut self, mouse_x: f32, cx: &mut Context<Self>) {
        let geo = self.h_scroll_geometry();
        let target_thumb_x = (mouse_x - geo.thumb_w / 2.0).clamp(0.0, geo.track_free);
        self.scroll_x = (target_thumb_x / geo.track_free * geo.max_scroll).clamp(0.0, geo.max_scroll);
        self.clamp_scroll(self.grid_size.get());
        cx.notify();
    }

    /// A minimap of the whole open channel, squeezed to fit the panel's
    /// own width regardless of how long the pattern actually is (unlike
    /// the grid itself, which is pixel-accurate at `STEP_W` per step) —
    /// Allodium-tauri's own `PianoRollOverview.svelte`
    /// (`<MiniNoteRoll fit="stretch">` + a lighter overlay rectangle for
    /// the visible fraction), and the same squeeze the Channel Rack's own
    /// mini piano-roll preview (`channel_rack/row.rs`'s `render_piano_
    /// preview`) uses for its *pitch* axis already, just applied to the
    /// *time* axis here too. The bright window rectangle is exactly
    /// `h_scroll_geometry`'s own thumb — dragging it or clicking the
    /// strip's background behind it scrolls the grid exactly like the
    /// horizontal scrollbar below, just with real note content as the
    /// backdrop instead of a bare track, for VS Code-minimap-style
    /// at-a-glance navigation in a long pattern.
    pub(super) fn render_overview(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const NOTE_H: f32 = 3.0;

        let geo = self.h_scroll_geometry();
        let pattern_length = self.pattern_length_steps.max(1) as f64;

        let pitch_min = self.notes.iter().map(|n| n.pitch).min().unwrap_or(60);
        let pitch_max = self.notes.iter().map(|n| n.pitch).max().unwrap_or(60);
        let span = (pitch_max - pitch_min).max(12) as f32;
        let mid = (pitch_min + pitch_max) as f32 / 2.0;
        let lo = mid - span / 2.0;

        let mut strip = div()
            .id("pr-overview")
            .relative()
            .h(px(OVERVIEW_H))
            .w_full()
            .flex_shrink_0()
            .overflow_hidden()
            .bg(theme::STEP_OFF_GREY)
            .border_b_1()
            .border_color(theme::BORDER)
            .cursor_pointer()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| this.jump_h_scroll(f32::from(ev.position.x), cx)),
            );

        for (note_idx, note) in self.notes.iter().enumerate() {
            let x = (note.start / pattern_length) as f32 * geo.viewport_w;
            let w = ((note.length / pattern_length) as f32 * geo.viewport_w).max(1.0);
            let t = ((note.pitch as f32 - lo) / span).clamp(0.0, 1.0);
            let y = (1.0 - t) * (OVERVIEW_H - NOTE_H);
            let color: gpui::Hsla = if note.color != 0 { gpui::rgb(note.color).into() } else { theme::accent() };
            strip = strip.child(
                div().id(("pr-overview-note", note_idx)).absolute().left(px(x)).top(px(y)).w(px(w)).h(px(NOTE_H)).rounded(theme::RADIUS_XS).bg(color),
            );
        }

        strip.child(
            div()
                .id("pr-overview-window")
                .absolute()
                .left(px(geo.thumb_x))
                .top_0()
                .w(px(geo.thumb_w.max(4.0)))
                .h_full()
                .border_1()
                .border_color(theme::accent())
                .bg(theme::accent().alpha(0.15))
                .cursor_pointer()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                        this.begin_h_scroll_drag(f32::from(ev.position.x));
                        cx.stop_propagation();
                    }),
                ),
        )
    }

    /// Piano Roll ruler click-to-seek — the same gesture the Playlist's
    /// own ruler click-to-seek uses (`playlist.rs`'s `handle_ruler_click`),
    /// moving the playhead to wherever was clicked. Unlike the Playlist's
    /// version this isn't gated on a transport mode: `current_step()` (what
    /// this grid's own playhead and the Channel Rack's step highlight both
    /// read) is always meaningful, pattern-mode or song-mode alike, since
    /// it's the same wrapped `samples_played` clock either way
    /// (`ChannelRack::seek_transport_to_step`'s own doc comment).
    fn handle_ruler_click(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let x = f32::from(position.x - self.grid_origin.get().x) + self.scroll_x;
        let step = (x / STEP_W).max(0.0) as f64;
        self.channel_rack.update(cx, |rack, _| rack.seek_transport_to_step(step));
        cx.notify();
    }

    /// A numbered ruler above the grid — Allodium-tauri's own Playlist
    /// ruler (`playlist.rs`'s own `render`, numbering its bold per-bar
    /// gridlines). The grid itself still bands light/dark every beat (4
    /// steps, `paint::paint_grid`'s "Beat-group column banding"), but the
    /// numbers label *bars* (`beats_per_bar` beats each — the project's
    /// own time signature, same as the Playlist's own bars), one per bar
    /// at its first beat, not one per beat. Only the bars actually in
    /// view get a label, same virtualization the banding itself and the
    /// Playlist's own ruler both already use.
    pub(super) fn render_beat_ruler(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const STEPS_PER_BEAT: i32 = 4;
        let beats_per_bar = (self.channel_rack.read(cx).beats_per_bar() as i32).max(1);

        let viewport_w = f32::from(self.grid_size.get().width).max(0.0);
        let first_step = (self.scroll_x / STEP_W).floor().max(0.0) as i32;
        let last_step = ((self.scroll_x + viewport_w) / STEP_W).ceil() as i32;
        let first_bar = first_step / STEPS_PER_BEAT / beats_per_bar;
        let last_bar = last_step / STEPS_PER_BEAT / beats_per_bar;

        let mut labels = div().absolute().left_0().top_0().size_full();
        for bar in first_bar..=last_bar {
            let x = step_to_x((bar * beats_per_bar * STEPS_PER_BEAT) as f64) - self.scroll_x;
            labels = labels.child(
                div()
                    .absolute()
                    .left(px(x + 3.0))
                    .top_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .text_size(px(9.0))
                    .text_color(theme::accent_dim())
                    .child(format!("{}", bar + 1)),
            );
        }

        div()
            .h(px(RULER_H))
            .w_full()
            .flex_shrink_0()
            .relative()
            .overflow_hidden()
            .cursor_pointer()
            .bg(theme::HEADER_BG)
            .border_b_1()
            .border_color(theme::BORDER)
            .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _, cx| this.handle_ruler_click(ev.position, cx)))
            .child(labels)
    }

    /// A thin draggable horizontal scrollbar for the grid, below it —
    /// the grid's own content is painted procedurally in a `canvas`
    /// (`paint::paint_grid`), not laid out as real wide DOM content GPUI
    /// could track with its own `Scrollbar`/`ScrollHandle` machinery (the
    /// `ChannelRack`'s own visible scrollbar, `channel_rack.rs`'s own
    /// `render` doc comment, relies on exactly that machinery, which
    /// isn't available here), so this one computes its own thumb size/
    /// position from `scroll_x`/`content_width()`/the grid's own tracked
    /// viewport width instead. Hidden (collapses to the bare track strip)
    /// once the content already fits, same as a native scrollbar would.
    pub(super) fn render_h_scrollbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const TRACK_H: f32 = 10.0;
        let geo = self.h_scroll_geometry();

        let track = div().id("pr-hscroll-track").relative().h(px(TRACK_H)).w_full().flex_shrink_0().bg(theme::HEADER_BG);
        if geo.content_w <= geo.viewport_w || geo.viewport_w <= 0.0 {
            return track;
        }

        track
            .cursor_pointer()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| this.jump_h_scroll(f32::from(ev.position.x), cx)),
            )
            .child(
                div()
                    .id("pr-hscroll-thumb")
                    .absolute()
                    .left(px(geo.thumb_x))
                    .top_0()
                    .w(px(geo.thumb_w))
                    .h(px(TRACK_H))
                    .rounded(theme::RADIUS_XS)
                    .bg(theme::BTN_BORDER)
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::accent_dim()))
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                            this.begin_h_scroll_drag(f32::from(ev.position.x));
                            cx.stop_propagation();
                        }),
                    ),
            )
    }
}
