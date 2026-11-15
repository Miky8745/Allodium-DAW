//! Design tokens, derived from the user's NixOS desktop config
//! (waybar/niri/swaync/wallpaper) per `daw-rewrite-starter/01-theme.md` and
//! the deeper visual-redesign audit in `docs/stage-08-visual-redesign.md`.
//! Every other module should import these constants rather than
//! hardcoding colors/radii/fonts.

use gpui::{Hsla, Pixels, px};

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

// Focus / accent.
pub const ACCENT: Hsla = Hsla { h: 201.0 / 360.0, s: 0.90, l: 0.75, a: 1.0 }; // #7fc8ff
pub const ACCENT_DIM: Hsla = Hsla { h: 201.0 / 360.0, s: 0.56, l: 0.52, a: 1.0 }; // #4a9bd4
pub const ACCENT_TEXT: Hsla = Hsla { h: 204.0 / 360.0, s: 0.45, l: 0.08, a: 1.0 }; // #0a1620

/// A rare, deliberate warm contrast note — not a second general-purpose
/// accent. Echoes the desktop wallpaper's own orange-vs-cyan drama
/// (`~/Background/gargantua-black-...jpg`: a warm amber gas giant against
/// a cool cyan glow). This is the exact color the Piano Roll/Playlist
/// delete-flash animations already used as a magic number; naming it here
/// makes that choice visible instead of accidental. Keep its usage rare.
pub const WARM_ACCENT: Hsla = Hsla { h: 14.0 / 360.0, s: 0.98, l: 0.64, a: 1.0 }; // #ff6a3d

// Semantic (unchanged genre conventions).
pub const METER_GREEN: Hsla = Hsla { h: 120.0 / 360.0, s: 0.6, l: 0.45, a: 1.0 };
pub const METER_YELLOW: Hsla = Hsla { h: 50.0 / 360.0, s: 0.85, l: 0.55, a: 1.0 };
pub const METER_RED: Hsla = Hsla { h: 4.0 / 360.0, s: 0.75, l: 0.55, a: 1.0 };
pub const RECORD_RED: Hsla = Hsla { h: 4.0 / 360.0, s: 0.75, l: 0.55, a: 1.0 };
pub const MUTE_RED: Hsla = Hsla { h: 4.0 / 360.0, s: 0.75, l: 0.55, a: 1.0 };

// Borders / chrome.
pub const BORDER: Hsla = Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.06 };
pub const FOCUS_RING: Hsla = ACCENT;
pub const FOCUS_RING_WIDTH: Pixels = px(2.0);

// Radii.
pub const RADIUS_SM: Pixels = px(6.0);
pub const RADIUS_MD: Pixels = px(10.0);
pub const RADIUS_LG: Pixels = px(14.0);

// Toolbar layout — back to two rows (see `docs/stage-08-visual-redesign.md`
// for why the row was originally dropped: it was *empty*; the toolbar
// rebuild documented in `docs/stage-09-toolbar.md` gave row 2 real content
// — panel toggles, pattern/snap pickers, modifier-key badges — so the
// second row earns its place back). Each row sizes to this height; the
// toolbar's total height is just the two rows stacked, not a separate
// constant.
pub const TOOLBAR_ROW_H: Pixels = px(34.0);

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
