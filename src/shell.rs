//! The app shell: a two-row toolbar (see `src/toolbar.rs`) + a floating-
//! window workspace. Channel Rack, Piano Roll, Playlist, and Mixer are
//! draggable/resizable/closable panels (see `src/floating_panel.rs`),
//! reopened via the toolbar's toggle pills — the original app's
//! "window-based" model, redesigned fresh rather than ported. See
//! `docs/stage-08-visual-redesign.md` and `docs/stage-09-toolbar.md`.

use std::sync::Arc;

use gpui::component::input::{InputEvent, InputState};
use gpui::component::{Icon, Sizable};
use gpui::{
    AppContext, ClickEvent, Context, DragMoveEvent, Entity, FocusHandle, Focusable, IntoElement,
    ModifiersChangedEvent, MouseDownEvent, ParentElement, Pixels, Point, Render, Styled, Window, div,
    linear_color_stop, linear_gradient, point, prelude::*, px, size,
};

use crate::audio::AudioEngine;
use crate::channel_rack::ChannelRack;
use crate::file_browser::FileBrowser;
use crate::floating_panel::{
    self, PanelDrag, PanelDragKind, PanelGeometry, PanelKind, ResizeEdge, ShellDragMarker, MIN_PANEL_SIZE,
};
use crate::mixer::Mixer;
use crate::perf::FrameStats;
use crate::piano_roll::PianoRoll;
use crate::playlist::Playlist;
use crate::preferences::Preferences;
use crate::theme;

struct PanelGeometries {
    channel_rack: PanelGeometry,
    piano_roll: PanelGeometry,
    playlist: PanelGeometry,
    mixer: PanelGeometry,
}

impl PanelGeometries {
    fn get(&self, kind: PanelKind) -> PanelGeometry {
        match kind {
            PanelKind::ChannelRack => self.channel_rack,
            PanelKind::PianoRoll => self.piano_roll,
            PanelKind::Playlist => self.playlist,
            PanelKind::Mixer => self.mixer,
        }
    }

    fn get_mut(&mut self, kind: PanelKind) -> &mut PanelGeometry {
        match kind {
            PanelKind::ChannelRack => &mut self.channel_rack,
            PanelKind::PianoRoll => &mut self.piano_roll,
            PanelKind::Playlist => &mut self.playlist,
            PanelKind::Mixer => &mut self.mixer,
        }
    }
}

/// Initial layout roughly mirrors the previous fixed stacked order
/// (Channel Rack, then Piano Roll, then Playlist down the left; Mixer to
/// the right) — familiar on first launch, freely rearrangeable after.
impl Default for PanelGeometries {
    fn default() -> Self {
        let geo = |x: f32, y: f32, w: f32, h: f32| PanelGeometry {
            position: point(px(x), px(y)),
            size: size(px(w), px(h)),
            open: true,
            maximized: false,
            pre_maximize: None,
        };
        // Piano Roll and Playlist start maximized — their own geometry
        // here is just what `restore` snaps back to if un-maximized, not
        // what's actually shown on first launch.
        let maximized_geo = |x: f32, y: f32, w: f32, h: f32| PanelGeometry { maximized: true, ..geo(x, y, w, h) };
        Self {
            channel_rack: geo(16.0, 16.0, 860.0, 260.0),
            piano_roll: maximized_geo(16.0, 292.0, 860.0, 270.0),
            playlist: maximized_geo(16.0, 578.0, 860.0, 220.0),
            mixer: geo(892.0, 16.0, 380.0, 500.0),
        }
    }
}

pub struct AppShell {
    frame_stats: FrameStats,
    pub(crate) channel_rack: Entity<ChannelRack>,
    piano_roll: Entity<PianoRoll>,
    playlist: Entity<Playlist>,
    mixer: Entity<Mixer>,
    /// Docked to the left edge, not a floating `PanelKind` — see
    /// `file_browser.rs`'s own module doc for why.
    file_browser: Entity<FileBrowser>,
    file_browser_open: bool,
    geometry: PanelGeometries,
    z_order: Vec<PanelKind>,
    drag: Option<PanelDrag>,
    tempo_drag: Option<TempoDrag>,
    /// Whether the *current* `drag`/`tempo_drag` has actually crossed
    /// GPUI's own drag threshold and become a real `cx.active_drag` (see
    /// the root div's `on_drag_move` below) — not just a mouse-down that
    /// might turn out to be a plain click. Needed because `begin_drag`
    /// sets `drag`/`tempo_drag` *before* that threshold is crossed, so
    /// polling `cx.has_active_drag()` alone to decide "the drag ended"
    /// would immediately — and wrongly — cancel every drag the instant
    /// after it starts, while the pointer is still within the 2px
    /// threshold and `active_drag` hasn't been set yet.
    drag_confirmed: bool,
    pub(crate) toolbar: crate::toolbar::ToolbarState,
    /// Created once here rather than inside the Preferences dialog's own
    /// `open_dialog` builder — that builder can re-run on every rebuild
    /// while the dialog is open, and a fresh `cx.new(...)` each time would
    /// reset the selected tab back to General.
    pub(crate) preferences: Entity<Preferences>,
    /// Backs the toolbar's double-click-to-type-a-BPM field. Created once
    /// here (not inside `render_toolbar`, which reruns every frame) so its
    /// text and focus survive across renders while editing is open.
    pub(crate) tempo_input: Entity<InputState>,
    /// Tracked on the root div so the Space-to-play/stop keybinding
    /// (`toolbar::TogglePlayback`) has somewhere to dispatch to by default
    /// — nothing else claims focus until a user action does (e.g. editing
    /// the BPM field), so this is where key dispatch starts at launch.
    focus_handle: FocusHandle,
}

/// Dragging the BPM pill vertically nudges tempo by whole BPM per pixel of
/// travel — the same gesture the scroll wheel offers, just continuous
/// instead of stepped, and (unlike scroll) guaranteed to actually reach
/// this code: trackpads and some mice report wheel deltas GPUI's
/// `ScrollEvent` doesn't always see on this platform.
struct TempoDrag {
    start_mouse_y: Pixels,
    start_tempo: f64,
}

impl AppShell {
    pub fn new(engine: Arc<AudioEngine>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let channel_rack = cx.new(|cx| ChannelRack::new(engine, window, cx));
        cx.subscribe(&channel_rack, |this: &mut Self, _rack, event: &crate::channel_rack::ChannelRackEvent, cx| match event {
            crate::channel_rack::ChannelRackEvent::HoverInfo(name, value) => {
                this.toolbar.hover = Some((name, value.clone()));
                cx.notify();
            }
        })
        .detach();
        let piano_roll = cx.new(|cx| PianoRoll::new(channel_rack.clone(), window, cx));
        let playlist = cx.new(|cx| Playlist::new(channel_rack.clone(), window, cx));
        let mixer = cx.new(|cx| Mixer::new(channel_rack.clone(), window, cx));
        let file_browser = cx.new(|cx| FileBrowser::new(window, cx));
        let preferences = cx.new(|_| Preferences::new());
        let tempo_input = cx.new(|cx| InputState::new(window, cx));
        cx.subscribe(&tempo_input, |this: &mut Self, _input, event: &InputEvent, cx| match event {
            InputEvent::PressEnter { .. } | InputEvent::Blur => this.commit_tempo_edit(cx),
            _ => {}
        })
        .detach();
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            frame_stats: FrameStats::new(),
            channel_rack,
            piano_roll,
            playlist,
            mixer,
            file_browser,
            file_browser_open: true,
            geometry: PanelGeometries::default(),
            z_order: PanelKind::ALL.to_vec(),
            drag: None,
            tempo_drag: None,
            drag_confirmed: false,
            toolbar: crate::toolbar::ToolbarState::default(),
            preferences,
            tempo_input,
            focus_handle,
        }
    }

    pub(crate) fn begin_tempo_drag(&mut self, mouse_y: Pixels, cx: &mut Context<Self>) {
        if self.toolbar.editing_tempo {
            return;
        }
        let start_tempo = self.channel_rack.read(cx).tempo();
        self.tempo_drag = Some(TempoDrag { start_mouse_y: mouse_y, start_tempo });
        self.drag_confirmed = false;
    }

    /// Clears a pending `drag`/`tempo_drag` that never actually turned into
    /// a real drag — a plain click that didn't cross GPUI's own 2px
    /// threshold, so `cx.active_drag` never became `Some` and the root
    /// div's `on_drag_move` never got a chance to run. Wired to
    /// `on_mouse_up` on the same element `begin_drag`/`begin_tempo_drag`
    /// started from, which (for a click that short) is still reliably
    /// hovered at release time. A *confirmed* drag's own end is handled
    /// separately, by polling `cx.has_active_drag()` once `drag_confirmed`
    /// is true — see that field's own doc comment.
    pub(crate) fn cancel_pending_click(&mut self, cx: &mut Context<Self>) {
        if self.drag_confirmed {
            return;
        }
        if self.drag.take().is_some() || self.tempo_drag.take().is_some() {
            cx.notify();
        }
    }

    fn update_tempo_drag(&mut self, mouse_y: Pixels, cx: &mut Context<Self>) {
        let Some(drag) = &self.tempo_drag else { return };
        // Up is higher tempo, matching the scroll handler's sign convention.
        let raw = f32::from(drag.start_mouse_y - mouse_y) as f64;
        // Accelerating, not 1:1: GPUI exposes no pointer-warp/lock API on
        // this platform (confirmed against its own window/platform source —
        // there's nothing past a read-only `is_cursor_visible`), and Wayland
        // itself doesn't let an ordinary client warp the system cursor the
        // way X11's `XWarpPointer` could. "Teleport the cursor to the top
        // when it hits the bottom" genuinely isn't implementable here, so
        // this is the practical substitute: the covered range grows with
        // distance already dragged, so the full 20-999 BPM span fits inside
        // a few hundred pixels of real travel instead of needing (literally)
        // a thousand, and a finger's reach of screen rarely runs out.
        let accelerated = raw.signum() * (raw.abs() + raw.abs().powf(1.6) / 40.0);
        let next = drag.start_tempo + accelerated;
        self.channel_rack.update(cx, |cr, cx| cr.set_tempo(next, cx));
    }

    fn end_tempo_drag(&mut self) {
        self.tempo_drag = None;
    }

    pub(crate) fn start_edit_tempo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tempo_drag = None;
        let tempo = self.channel_rack.read(cx).tempo();
        self.tempo_input.update(cx, |s, cx| s.set_value(format!("{tempo:.1}"), window, cx));
        self.toolbar.editing_tempo = true;
        self.tempo_input.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    fn commit_tempo_edit(&mut self, cx: &mut Context<Self>) {
        if !self.toolbar.editing_tempo {
            return;
        }
        if let Ok(tempo) = self.tempo_input.read(cx).value().trim().parse::<f64>() {
            self.channel_rack.update(cx, |cr, cx| cr.set_tempo(tempo, cx));
        }
        self.toolbar.editing_tempo = false;
        cx.notify();
    }

    fn is_frontmost(&self, kind: PanelKind) -> bool {
        self.z_order.last() == Some(&kind)
    }

    fn focus_panel(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        self.z_order.retain(|k| *k != kind);
        self.z_order.push(kind);
        cx.notify();
    }

    /// Closed → open + focus; open-but-not-frontmost → bring to front
    /// (stays open); open-and-already-frontmost → close. The original's
    /// exact 3-state toolbar semantics.
    fn toggle_panel(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        let open = self.geometry.get(kind).open;
        if !open {
            self.geometry.get_mut(kind).open = true;
            self.focus_panel(kind, cx);
        } else if !self.is_frontmost(kind) {
            self.focus_panel(kind, cx);
        } else {
            self.geometry.get_mut(kind).open = false;
            cx.notify();
        }
    }

    /// The rect actually drawn for `kind` this frame: its stored geometry
    /// normally, or pinned to fill `workspace_size` (at the workspace's
    /// own origin, since panels are positioned relative to it rather than
    /// the window) while maximized. Mirrors Allodium-tauri's own
    /// `FloatingWindow`, which maximizes into its workspace zone rather
    /// than the raw window bounds, never covering the toolbar.
    fn display_geometry(&self, kind: PanelKind, workspace_size: gpui::Size<Pixels>) -> PanelGeometry {
        let geo = self.geometry.get(kind);
        if geo.maximized {
            PanelGeometry { position: point(px(0.0), px(0.0)), size: workspace_size, ..geo }
        } else {
            geo
        }
    }

    /// Shows/hides the docked File Browser sidebar — a plain 2-state
    /// toggle (unlike `toggle_panel`'s 3-state floating-window semantics),
    /// since a dock has no z-order to raise/lower.
    fn toggle_file_browser(&mut self, cx: &mut Context<Self>) {
        self.file_browser_open = !self.file_browser_open;
        cx.notify();
    }

    /// Mirrors `toggle_pill`'s visual treatment (icon-only, accent-tinted
    /// when active) for the one dock this shell has, without reusing
    /// `PanelKind`'s z-order-aware open/frontmost states that don't apply
    /// to a sidebar.
    pub(crate) fn file_browser_toggle_pill(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.file_browser_open;
        let mut pill = div()
            .id("file-browser-toggle")
            .w(px(34.0))
            .h(px(34.0))
            .rounded(theme::RADIUS_SM)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(Icon::new(crate::file_browser::ICON).small())
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_file_browser(cx)));
        pill = if open {
            pill.bg(crate::file_browser::IDENTITY_COLOR.alpha(0.14)).text_color(crate::file_browser::IDENTITY_COLOR)
        } else {
            pill.text_color(theme::BTN_TEXT).hover(|s| s.bg(theme::BTN_HOVER).text_color(theme::MAIN_TEXT))
        };
        pill
    }

    fn close_panel(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        self.geometry.get_mut(kind).open = false;
        cx.notify();
    }

    /// Mirrors Allodium-tauri's own `FloatingWindow.toggleMaximize`: stash
    /// the current rect on the way in, restore it on the way out, so
    /// toggling off snaps back to exactly where the panel was rather than
    /// recomputing a default layout.
    fn toggle_maximize(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        let geo = self.geometry.get_mut(kind);
        if geo.maximized {
            if let Some((position, size)) = geo.pre_maximize.take() {
                geo.position = position;
                geo.size = size;
            }
            geo.maximized = false;
        } else {
            geo.pre_maximize = Some((geo.position, geo.size));
            geo.maximized = true;
        }
        self.focus_panel(kind, cx);
    }

    fn begin_drag(&mut self, kind: PanelKind, drag_kind: PanelDragKind, mouse: Point<Pixels>, cx: &mut Context<Self>) {
        let geo = self.geometry.get(kind);
        // Allodium-tauri's own `onDragStart`/`onResizeStart` both no-op
        // while maximized — there's nothing sensible for a move or resize
        // to do when the rect is pinned to the workspace, and the drag
        // start position itself (the stashed pre-maximize rect) isn't
        // where the panel currently visually is.
        if geo.maximized {
            return;
        }
        self.drag = Some(PanelDrag { kind: drag_kind, panel: kind, start_mouse: mouse, start_position: geo.position, start_size: geo.size });
        self.drag_confirmed = false;
        self.focus_panel(kind, cx);
    }

    fn update_drag(&mut self, mouse: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.drag else { return };
        let dx = mouse.x - drag.start_mouse.x;
        let dy = mouse.y - drag.start_mouse.y;
        let geo = self.geometry.get_mut(drag.panel);
        match drag.kind {
            PanelDragKind::Move => {
                geo.position.x = (drag.start_position.x + dx).max(px(0.0));
                geo.position.y = (drag.start_position.y + dy).max(px(0.0));
            }
            PanelDragKind::Resize(edge) => {
                // The right/bottom sides just grow or shrink `size` in
                // place; left/top also have to slide `position` to keep
                // the *opposite* edge fixed, the same way a window
                // manager's own edge/corner resize works — mirrors the
                // `Move` arm's own `>= 0` clamp, since dragging either of
                // these two is otherwise indistinguishable from moving
                // the panel while also resizing it.
                if edge.right {
                    geo.size.width = (drag.start_size.width + dx).max(MIN_PANEL_SIZE.width);
                }
                if edge.bottom {
                    geo.size.height = (drag.start_size.height + dy).max(MIN_PANEL_SIZE.height);
                }
                if edge.left {
                    let new_width = (drag.start_size.width - dx).max(MIN_PANEL_SIZE.width);
                    geo.position.x = (drag.start_position.x + (drag.start_size.width - new_width)).max(px(0.0));
                    geo.size.width = new_width;
                }
                if edge.top {
                    let new_height = (drag.start_size.height - dy).max(MIN_PANEL_SIZE.height);
                    geo.position.y = (drag.start_position.y + (drag.start_size.height - new_height)).max(px(0.0));
                    geo.size.height = new_height;
                }
            }
        }
        cx.notify();
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            cx.notify();
        }
    }

    /// Icon-only, borderless — Allodium-tauri's own `.tb-pb` panel-toggle
    /// treatment, not a labeled pill. Three states instead of the
    /// original's two: closed (neutral), open-but-behind-another-panel
    /// (a faint accent tint — this rewrite's own floating-window z-order
    /// concept, which the original's always-only-one-visible-window model
    /// never needed), and frontmost (full accent + glow).
    pub(crate) fn toggle_pill(&self, kind: PanelKind, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.geometry.get(kind).open;
        let frontmost = open && self.is_frontmost(kind);
        let mut pill = div()
            .id(("panel-toggle", kind as u8 as usize))
            .w(px(34.0))
            .h(px(34.0))
            .rounded(theme::RADIUS_SM)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(Icon::new(kind.icon()).small())
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_panel(kind, cx)));
        let identity = kind.identity_color();
        pill = if frontmost {
            pill.bg(identity.alpha(0.14)).text_color(identity).shadow(crate::toolbar::glow(identity))
        } else if open {
            pill.bg(identity.alpha(0.08)).text_color(identity).hover(|s| s.bg(theme::BTN_HOVER))
        } else {
            pill.text_color(theme::BTN_TEXT).hover(|s| s.bg(theme::BTN_HOVER).text_color(theme::MAIN_TEXT))
        };
        pill
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.frame_stats.tick();
        window.request_animation_frame();
        cx.notify();

        // GPUI auto-releases `cx.active_drag` on any mouse-up system-wide
        // (its own `dispatch_mouse_event`, unconditionally, before this
        // view even redraws) — so by the next frame after a release, this
        // is the reliable signal that a *confirmed* drag ended, standing
        // in for the `on_mouse_up`/`on_mouse_up_out` this root div can no
        // longer use for that now that drag tracking has moved to
        // `on_drag_move` (see the root div's own comment below for why).
        // Gated on `drag_confirmed` — see that field's doc comment for why
        // this can't just check `!cx.has_active_drag()` unconditionally.
        if self.drag_confirmed && !cx.has_active_drag() {
            self.end_drag(cx);
            self.end_tempo_drag();
            self.drag_confirmed = false;
        }

        let piano_roll_channel_name = {
            let rack = self.channel_rack.read(cx);
            rack.selected_channel_id().and_then(|id| rack.channels().iter().find(|c| c.id == id)).map(|c| {
                if c.name.is_empty() { "Drop sample".to_string() } else { c.name.clone() }
            })
        };
        let piano_roll_title = match &piano_roll_channel_name {
            Some(name) => format!("Piano Roll — {name}"),
            None => "Piano Roll".to_string(),
        };

        let dock_w = if self.file_browser_open { theme::FILE_BROWSER_DOCK_W } else { px(0.0) };
        let workspace_size =
            size(window.viewport_size().width - dock_w, window.viewport_size().height - theme::TOOLBAR_TOTAL_H);

        let available_mixer_h = self.display_geometry(PanelKind::Mixer, workspace_size).size.height - px(26.0);
        self.mixer.update(cx, |m, cx| m.set_fader_height((available_mixer_h - px(140.0)).max(px(40.0)), cx));

        let mut workspace = div().absolute().left_0().top_0().size_full();
        for kind in self.z_order.clone() {
            let geo = self.display_geometry(kind, workspace_size);
            if !geo.open {
                continue;
            }
            let focused = self.is_frontmost(kind);
            let dragging_this = self.drag.map(|d| d.panel == kind).unwrap_or(false);
            let title = match kind {
                PanelKind::PianoRoll => piano_roll_title.clone(),
                other => other.label().to_string(),
            };
            let body: gpui::AnyElement = match kind {
                PanelKind::ChannelRack => self.channel_rack.clone().into_any_element(),
                PanelKind::PianoRoll => self.piano_roll.clone().into_any_element(),
                PanelKind::Playlist => self.playlist.clone().into_any_element(),
                PanelKind::Mixer => self.mixer.clone().into_any_element(),
            };
            workspace = workspace.child(floating_panel::render_panel(
                kind,
                title,
                geo,
                focused,
                dragging_this,
                body,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| this.focus_panel(kind, cx)),
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| this.begin_drag(kind, PanelDragKind::Move, ev.position, cx)),
                cx.listener(move |this, (edge, ev): &(ResizeEdge, MouseDownEvent), _, cx| {
                    this.begin_drag(kind, PanelDragKind::Resize(*edge), ev.position, cx)
                }),
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| this.cancel_pending_click(cx)),
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| this.cancel_pending_click(cx)),
                cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_maximize(kind, cx)),
                cx.listener(move |this, _: &ClickEvent, _, cx| this.close_panel(kind, cx)),
            ));
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::BG_APP)
            .font_family(theme::FONT_UI)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_toggle_playback))
            .on_modifiers_changed(cx.listener(|this, ev: &ModifiersChangedEvent, _, cx| {
                this.toolbar.modifiers = ev.modifiers;
                cx.notify();
            }))
            // Drag tracking for both click-drag gestures (floating panels,
            // the BPM pill) rides GPUI's own drag-and-drop machinery
            // (`on_drag` on the originating element in `floating_panel.rs`/
            // `toolbar.rs`, `on_drag_move` here) rather than plain
            // `on_mouse_move` on this outer root: `on_drag_move` is
            // dispatched by payload type with no hitbox/hover check at
            // all, so — unlike `on_mouse_move` — it isn't blocked by a
            // panel's `occlude()` sitting between the pointer and this
            // root div, and it keeps tracking even while the pointer
            // wanders outside the workspace's own bounds. See
            // `ShellDragMarker` in `floating_panel.rs`.
            .on_drag_move::<ShellDragMarker>(cx.listener(|this, event: &DragMoveEvent<ShellDragMarker>, _, cx| {
                // Reaching here at all means `cx.active_drag` is genuinely
                // `Some` for this payload type — the real signal (as
                // opposed to a mouse-down that might still turn out to be
                // a plain click) that this is a confirmed drag. See
                // `drag_confirmed`'s own doc comment.
                this.drag_confirmed = true;
                let position = event.event.position;
                if this.drag.is_some() {
                    this.update_drag(position, cx);
                }
                if this.tempo_drag.is_some() {
                    this.update_tempo_drag(position.y, cx);
                }
            }))
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    // Docked to the left edge, IDE-style — not a floating
                    // `PanelKind` (see `file_browser.rs`'s module doc) —
                    // so it sits outside the workspace's own absolute-
                    // positioned panel stack entirely, shrinking it rather
                    // than overlapping it.
                    .when(self.file_browser_open, |d| {
                        d.child(div().flex_shrink_0().w(theme::FILE_BROWSER_DOCK_W).h_full().child(self.file_browser.clone()))
                    })
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            // A faint directional wash of the active accent, not a
                            // flat fill — GPUI has no backdrop-filter/blur, so the
                            // floating panels' translucent fill (`floating_panel.rs`)
                            // needs *something* with visible variation behind it to
                            // read as glass rather than just a slightly different
                            // flat color.
                            .bg(linear_gradient(135.0, linear_color_stop(theme::accent().alpha(0.05), 0.0), linear_color_stop(theme::CANVAS_BG, 0.5)))
                            .child(workspace),
                    ),
            )
    }
}
