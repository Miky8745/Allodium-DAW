//! The one drag-tracking slot every click-and-drag control in the
//! Channel Rack shares — mirrors `ChannelRack.svelte`'s own single
//! `onWindowMousemove` driving both the name-column resize and (via
//! `Dial.svelte`'s own identical listener) every dial, rather than a
//! separate drag state per control.

use gpui::{Context, Pixels, Point};

use super::{ChannelRack, DIAL_SENSITIVITY};

/// Which per-channel knob a drag is turning — governs its range and reset
/// value. Matches `ChannelRow.svelte`'s own two `Dial`s exactly (same
/// sensitivity, same sweep).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum ParamKind {
    Pan,
    Volume,
}

impl ParamKind {
    pub(super) fn max(self) -> f64 {
        match self {
            ParamKind::Pan => 1.0,
            ParamKind::Volume => 2.0,
        }
    }

    pub(super) fn default_value(self) -> f64 {
        match self {
            ParamKind::Pan => 0.5,
            ParamKind::Volume => 0.8,
        }
    }

    // Pan sweeps the classic full-travel rotary pot (gap at the bottom);
    // Volume sweeps left→top→right so max lands on the right — both
    // straight off `ChannelRow.svelte`'s own two `<Dial>` usages.
    pub(super) fn sweep_start(self) -> f32 {
        match self {
            ParamKind::Pan => 210.0,
            ParamKind::Volume => 270.0,
        }
    }

    pub(super) fn sweep_degrees(self) -> f32 {
        match self {
            ParamKind::Pan => 300.0,
            ParamKind::Volume => 180.0,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ParamDrag {
    channel_id: u32,
    kind: ParamKind,
    start_mouse_y: f32,
    start_value: f64,
}

#[derive(Clone, Copy)]
pub(super) struct SwingDrag {
    start_mouse_y: f32,
    start_value: f64,
}

#[derive(Clone, Copy)]
pub(super) struct ColResizeDrag {
    start_x: f32,
    start_width: f32,
}

#[derive(Clone, Copy)]
pub(super) enum ActiveDrag {
    Param(ParamDrag),
    Swing(SwingDrag),
    ColResize(ColResizeDrag),
}

impl ChannelRack {
    pub(super) fn begin_param_drag(&mut self, channel_id: u32, kind: ParamKind, start_mouse_y: f32) {
        let start_value = self
            .project
            .channels
            .items
            .iter()
            .find(|c| c.id == channel_id)
            .map(|ch| match kind {
                ParamKind::Pan => ch.pan,
                ParamKind::Volume => ch.volume,
            })
            .unwrap_or(0.0);
        self.drag = Some(ActiveDrag::Param(ParamDrag { channel_id, kind, start_mouse_y, start_value }));
    }

    pub(super) fn reset_param(&mut self, channel_id: u32, kind: ParamKind, cx: &mut Context<Self>) {
        if let Some(ch) = self.project.channels.items.iter_mut().find(|c| c.id == channel_id) {
            match kind {
                ParamKind::Pan => ch.pan = kind.default_value(),
                ParamKind::Volume => ch.volume = kind.default_value(),
            }
        }
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    pub(super) fn begin_swing_drag(&mut self, start_mouse_y: f32) {
        self.drag = Some(ActiveDrag::Swing(SwingDrag { start_mouse_y, start_value: self.swing }));
    }

    pub(super) fn begin_col_resize(&mut self, start_x: f32) {
        self.drag = Some(ActiveDrag::ColResize(ColResizeDrag { start_x, start_width: self.name_col_width }));
    }

    pub(super) fn update_drag(&mut self, mouse: Point<Pixels>, cx: &mut Context<Self>) {
        match self.drag {
            Some(ActiveDrag::Param(drag)) => {
                let dy = drag.start_mouse_y - f32::from(mouse.y);
                let max = drag.kind.max();
                let new_value = (drag.start_value + (dy / DIAL_SENSITIVITY) as f64 * max).clamp(0.0, max);
                if let Some(ch) = self.project.channels.items.iter_mut().find(|c| c.id == drag.channel_id) {
                    match drag.kind {
                        ParamKind::Pan => ch.pan = new_value,
                        ParamKind::Volume => ch.volume = new_value,
                    }
                }
                self.push_snapshot();
                cx.notify();
            }
            Some(ActiveDrag::Swing(drag)) => {
                let dy = drag.start_mouse_y - f32::from(mouse.y);
                self.swing = (drag.start_value + (dy / DIAL_SENSITIVITY) as f64).clamp(0.0, 1.0);
                cx.notify();
            }
            Some(ActiveDrag::ColResize(drag)) => {
                let dx = f32::from(mouse.x) - drag.start_x;
                self.name_col_width = (drag.start_width + dx).clamp(80.0, 280.0);
                cx.notify();
            }
            None => {}
        }
    }

    pub(super) fn end_drag(&mut self) {
        // Only a `Param` drag touches persisted project state — swing is
        // cosmetic/local and the column width is view-only, same as the
        // original's own un-persisted `nameColWidth`.
        if let Some(ActiveDrag::Param(_)) = self.drag.take() {
            self.save();
        }
    }
}
