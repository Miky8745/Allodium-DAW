//! Design tokens, derived from the user's NixOS desktop config
//! (waybar/niri/swaync/wallpaper) per `daw-rewrite-starter/01-theme.md` and
//! the deeper visual-redesign audit in `docs/stage-08-visual-redesign.md`.
//! Every other module should import these constants rather than
//! hardcoding colors/radii/fonts.

use std::sync::RwLock;

use gpui::{App, Hsla, Pixels, px};

// `gpui::hsla()` isn't a `const fn`, so these are built as plain struct
// literals (same fields: hue/saturation/lightness/alpha, each 0.0-1.0)
// rather than going through it.

// Backgrounds (darkest to lightest).
pub const BG_APP: Hsla = Hsla { h: 226.0 / 360.0, s: 0.15, l: 0.11, a: 1.0 }; // #1a1c24
pub const BG_PANEL: Hsla = Hsla { h: 222.0 / 360.0, s: 0.13, l: 0.14, a: 1.0 }; // #22252e
// rgba(56,60,74,0.8) — corrected from an earlier 0.82 guess; verified
// against the live waybar.css value, not a secondhand summary.
pub const BG_PILL: Hsla = Hsla { h: 230.0 / 360.0, s: 0.12, l: 0.25, a: 0.8 };
pub const BG_PILL_HOVER: Hsla = Hsla { h: 228.0 / 360.0, s: 0.06, l: 0.52, a: 0.8 }; // rgba(124,129,140,0.8)
pub const BG_PILL_ACTIVE: Hsla = Hsla { h: 228.0 / 360.0, s: 0.10, l: 0.35, a: 0.85 }; // rgba(78,82,99,0.85)

// Pulled straight from Allodium-tauri's `src/app.css` — the toolbar/
// panel-chrome port (docs/mockups/toolbar-preferences-concept.html) draws
// a distinction the waybar-matched tokens above don't: a slightly darker
// "chrome" surface (toolbar, floating-panel borders) against a slightly
// lighter "canvas" surface (the workspace behind those panels). `BG_APP`
// already happens to equal the original's `--sidebar-bg` almost exactly,
// so it keeps that job; this adds the canvas tone plus the handful of
// surfaces the original app itself names as distinct colors.
pub const CANVAS_BG: Hsla = Hsla { h: 225.0 / 360.0, s: 0.162, l: 0.145, a: 1.0 }; // #1f222b
pub const SLATE: Hsla = Hsla { h: 230.6 / 360.0, s: 0.172, l: 0.204, a: 1.0 }; // #2b2e3d — menu section, BPM/POS/pattern stat pills
pub const HEADER_BG: Hsla = Hsla { h: 229.8 / 360.0, s: 0.171, l: 0.1375, a: 1.0 }; // #1d1f29 — floating-panel title bars
pub const BTN_BG: Hsla = Hsla { h: 227.8 / 360.0, s: 0.162, l: 0.1825, a: 1.0 }; // #272a36 — bordered icon-button fill
pub const BTN_BORDER: Hsla = Hsla { h: 228.5 / 360.0, s: 0.155, l: 0.251, a: 1.0 }; // #363a4a
pub const BTN_HOVER: Hsla = Hsla { h: 227.2 / 360.0, s: 0.166, l: 0.2255, a: 1.0 }; // #303443
pub const BTN_TEXT: Hsla = Hsla { h: 227.4 / 360.0, s: 0.157, l: 0.76275, a: 1.0 }; // #b9bdcc — rest-state icon/label color
pub const MAIN_TEXT: Hsla = Hsla { h: 225.0 / 360.0, s: 0.162, l: 0.8549, a: 1.0 }; // #d4d7e0 — hover/emphasis text

// Focus / accent — runtime-switchable (Preferences → Theme), not plain
// constants. Every call site below is a function (`theme::accent()`, not
// `theme::ACCENT`) that reads whichever `Palette` is currently active.
//
// A `Palette` specifies three deliberate hues, each with one job:
// `accent` (active/selected — transport, selected panel), `secondary` (a
// second, cooler role — the feature-toggle cluster, so that whole group
// doesn't just look like dimmer transport buttons), and `destructive`
// (record/mute/danger). Every other shade used on top of a fill
// (`accent_dim`, `accent_hover`, `accent_text`, and the `secondary_*`
// equivalents) is derived by a fixed ratio rather than hand-tuned per
// theme, so a new palette is "pick three colors that read clearly
// apart," not "retune eight dependent constants."
#[derive(Clone, Copy, PartialEq)]
pub struct Palette {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub accent: Hsla,
    pub secondary: Hsla,
    pub destructive: Hsla,
}

impl Palette {
    fn hover_of(c: Hsla) -> Hsla {
        Hsla { s: (c.s + 0.05).min(1.0), l: (c.l + 0.09).min(0.95), ..c }
    }
    fn dim_of(c: Hsla) -> Hsla {
        Hsla { s: c.s * 0.62, l: c.l * 0.69, ..c }
    }
    fn text_of(c: Hsla) -> Hsla {
        Hsla { s: c.s * 0.5, l: c.l * 0.11, ..c }
    }

    /// Lightened `accent`, for hovering over an already-accent-filled
    /// control (the Play button mid-playback, an active toggle) — without
    /// this, those controls go visually dead on hover since their resting
    /// state is already close to the brightest color in the palette.
    pub fn accent_hover(&self) -> Hsla {
        Self::hover_of(self.accent)
    }

    /// A muted `accent`, for secondary text/labels that still want to read
    /// as "this app's color" without competing with real content.
    pub fn accent_dim(&self) -> Hsla {
        Self::dim_of(self.accent)
    }

    /// A near-black tint of `accent`, for text/icons drawn on top of an
    /// accent-filled surface.
    pub fn accent_text(&self) -> Hsla {
        Self::text_of(self.accent)
    }

    /// `secondary`'s equivalent of `accent_hover`.
    pub fn secondary_hover(&self) -> Hsla {
        Self::hover_of(self.secondary)
    }

    /// `secondary`'s equivalent of `accent_dim`.
    pub fn secondary_dim(&self) -> Hsla {
        Self::dim_of(self.secondary)
    }

    /// `secondary`'s equivalent of `accent_text`.
    pub fn secondary_text(&self) -> Hsla {
        Self::text_of(self.secondary)
    }
}

/// The default palette — not a fresh invention but the exact accent/danger
/// hex pair pulled from Allodium-tauri's own `src/app.css` (`--accent:
/// #e6b07f`, the record color `#d26868`). The earlier default here
/// ("Glacier", a cool blue) was this rewrite's own invention and read as
/// cold next to the original; this restores the warm tone that was
/// actually being asked for all along.
pub const AMBER: Palette = Palette {
    id: "amber",
    name: "Amber",
    description: "The original warm accent — this is the one from the Tauri app.",
    accent: Hsla { h: 28.5 / 360.0, s: 0.673, l: 0.700, a: 1.0 }, // #e6b07f
    secondary: Hsla { h: 205.0 / 360.0, s: 0.55, l: 0.62, a: 1.0 }, // cool sky blue — echoes the old "warm vs cool" drama, now as a second role instead of the primary
    destructive: Hsla { h: 0.0 / 360.0, s: 0.542, l: 0.616, a: 1.0 }, // #d26868
};

/// A vivid purple accent with a warm coral danger color.
pub const VIOLET: Palette = Palette {
    id: "violet",
    name: "Violet",
    description: "Vivid purple accent with a warm coral for record and danger.",
    accent: Hsla { h: 248.6 / 360.0, s: 0.609, l: 0.688, a: 1.0 }, // #8d7fe0
    secondary: Hsla { h: 45.0 / 360.0, s: 0.60, l: 0.62, a: 1.0 }, // warm gold, complementary to the purple accent
    destructive: Hsla { h: 346.1 / 360.0, s: 0.644, l: 0.659, a: 1.0 }, // #e0708a
};

/// A muted green accent with a warm terracotta danger color.
pub const SAGE: Palette = Palette {
    id: "sage",
    name: "Sage",
    description: "Muted green accent with a warm terracotta for record and danger.",
    accent: Hsla { h: 149.4 / 360.0, s: 0.355, l: 0.630, a: 1.0 }, // #7fc2a0
    secondary: Hsla { h: 230.0 / 360.0, s: 0.45, l: 0.68, a: 1.0 }, // soft blue-violet, distinct from both the green accent and the terracotta danger color
    destructive: Hsla { h: 24.3 / 360.0, s: 0.701, l: 0.594, a: 1.0 }, // #e08a4f
};

/// The cool-blue palette this rewrite used before — kept as an opt-in
/// alternative, not the default.
pub const GLACIER: Palette = Palette {
    id: "glacier",
    name: "Glacier",
    description: "The cool blue from the last pass — kept as an option, not the default.",
    accent: Hsla { h: 201.0 / 360.0, s: 0.90, l: 0.75, a: 1.0 }, // #7fc8ff
    secondary: Hsla { h: 280.0 / 360.0, s: 0.45, l: 0.70, a: 1.0 }, // muted plum, apart from both the blue accent and the orange danger color
    destructive: Hsla { h: 14.0 / 360.0, s: 0.98, l: 0.64, a: 1.0 }, // #ff6a3d
};

/// Every palette offered in Preferences → Theme, in display order.
pub const PALETTES: &[Palette] = &[AMBER, VIOLET, SAGE, GLACIER];

pub fn default_palette() -> Palette {
    AMBER
}

static ACTIVE_PALETTE: RwLock<Palette> = RwLock::new(GLACIER);

pub fn active_palette() -> Palette {
    *ACTIVE_PALETTE.read().unwrap()
}

/// Switch the active palette and push it into `gpui-component`'s own
/// theme tokens, so every Button/Toggle/Toggle's checked state (driven by
/// `cx.theme().primary`/`.accent`/`.danger`) repaints along with this
/// app's own hand-styled chrome. Does not touch `Theme::change`'s dark/
/// light mode or any of the ~150 tokens this app doesn't route through.
pub fn apply_palette(cx: &mut App, palette: Palette) {
    *ACTIVE_PALETTE.write().unwrap() = palette;

    gpui::component::Theme::update(cx, |t| {
        t.radius = RADIUS_SM;
        t.background = BG_APP;
        t.border = BORDER;

        t.primary = palette.accent;
        t.primary_hover = palette.accent_hover();
        t.primary_active = BG_PILL_ACTIVE;
        t.primary_foreground = palette.accent_text();
        t.button_primary = palette.accent;
        t.button_primary_hover = palette.accent_hover();
        t.button_primary_active = BG_PILL_ACTIVE;
        t.button_primary_foreground = palette.accent_text();

        t.accent = palette.accent;
        t.accent_foreground = palette.accent_text();

        t.danger = palette.destructive;
        t.danger_hover = palette.destructive.alpha(0.8);
        t.danger_active = palette.destructive.alpha(0.6);
        t.danger_foreground = palette.accent_text();
        t.button_danger = palette.destructive;
        t.button_danger_hover = palette.destructive.alpha(0.8);
        t.button_danger_active = palette.destructive.alpha(0.6);
        t.button_danger_foreground = palette.accent_text();
    });
}

pub fn accent() -> Hsla {
    active_palette().accent
}
pub fn accent_hover() -> Hsla {
    active_palette().accent_hover()
}
pub fn accent_dim() -> Hsla {
    active_palette().accent_dim()
}
pub fn accent_text() -> Hsla {
    active_palette().accent_text()
}
pub fn warm_accent() -> Hsla {
    active_palette().destructive
}
pub fn destructive() -> Hsla {
    active_palette().destructive
}

pub fn secondary() -> Hsla {
    active_palette().secondary
}
pub fn secondary_hover() -> Hsla {
    active_palette().secondary_hover()
}
pub fn secondary_dim() -> Hsla {
    active_palette().secondary_dim()
}
pub fn secondary_text() -> Hsla {
    active_palette().secondary_text()
}

// Semantic (unchanged genre conventions, reserved for real metering).
pub const METER_GREEN: Hsla = Hsla { h: 120.0 / 360.0, s: 0.6, l: 0.45, a: 1.0 };
pub const METER_YELLOW: Hsla = Hsla { h: 50.0 / 360.0, s: 0.85, l: 0.55, a: 1.0 };

// Borders / chrome.
// Was a translucent white hairline; Allodium-tauri's own `--explorer-
// border`/`--sidebar-border` is this one solid slate instead (#333747),
// used consistently everywhere in that app a hairline shows up.
pub const BORDER: Hsla = Hsla { h: 228.0 / 360.0, s: 0.150, l: 0.2392, a: 1.0 };

// Radii — one consistent scale instead of ad hoc per-widget px() literals
// (the thing that made small controls read as sharp-cornered "ImGUI"
// chrome even though a radius was technically being applied: 3-4px is
// imperceptible as rounding on a 16-20px control, it just reads as a
// rendering imprecision). `RADIUS_XS` is the new floor for anything
// control-sized; nothing in the app should go below it.
pub const RADIUS_XS: Pixels = px(5.0);
pub const RADIUS_SM: Pixels = px(8.0);
pub const RADIUS_MD: Pixels = px(12.0);
pub const RADIUS_LG: Pixels = px(16.0);

// Toolbar layout — back to two rows (see `docs/stage-08-visual-redesign.md`
// for why the row was originally dropped: it was *empty*; the toolbar
// rebuild documented in `docs/stage-09-toolbar.md` gave row 2 real content
// — panel toggles, pattern/snap pickers, modifier-key badges — so the
// second row earns its place back). Each row sizes to this height; the
// toolbar's total height is just the two rows stacked, not a separate
// constant.
//
// 34px read as thin for how much live-session-critical control sits in
// it (transport, tempo, position, panel switching) — at a glance it was
// under 8% of a 900px-tall window. 50px is Allodium-tauri's own
// `.toolbar { grid-template-rows: 50px 50px; }` value, not a fresh
// guess: restoring it both answers "make it taller" and brings this
// back in line with the original.
pub const TOOLBAR_ROW_H: Pixels = px(50.0);
/// Two stacked rows (see above) — the workspace's maximize-fill geometry
/// needs this as a single figure rather than re-deriving `TOOLBAR_ROW_H *
/// 2.0` at each call site.
pub const TOOLBAR_TOTAL_H: Pixels = px(100.0);

/// The desktop's actual installed/used font — confirmed via a direct
/// config audit (waybar, swaync, wlogout, alacritty all pin this exact
/// name). The earlier "JetBrains Mono" guess was wrong: that family isn't
/// installed anywhere on this machine and silently falls back. This is
/// the Nerd Font *Mono* variant specifically, so Nerd Font glyphs render
/// at a fixed cell width alongside text — load-bearing for the toolbar/
/// panel-title icon-plus-label treatment this redesign introduces.
pub const FONT_UI: &str = "JetBrainsMono Nerd Font Mono";

/// A deterministic, distinguishable color per Playlist pattern — no new
/// persisted field needed, just a hash of the pattern's own id. Golden-
/// ratio hue stepping spreads consecutive ids far apart in hue even
/// though the ids themselves are sequential. Saturation/lightness match
/// `ACCENT_DIM` so every pattern color reads as "a hue from this app's
/// palette," not an arbitrary rainbow.
pub fn pattern_color(pattern_id: u32) -> Hsla {
    const GOLDEN_RATIO_CONJUGATE: f32 = 0.618_034;
    let hue = ((pattern_id as f32) * GOLDEN_RATIO_CONJUGATE).fract();
    Hsla { h: hue, s: 0.56, l: 0.52, a: 1.0 }
}
