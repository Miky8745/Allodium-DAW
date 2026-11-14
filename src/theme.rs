//! Design tokens, derived from the user's NixOS desktop config
//! (waybar/niri) per `daw-rewrite-starter/01-theme.md`. Every other module
//! should import these constants rather than hardcoding colors/radii/fonts.

use gpui::{Hsla, Pixels, px};

// `gpui::hsla()` isn't a `const fn`, so these are built as plain struct
// literals (same fields: hue/saturation/lightness/alpha, each 0.0-1.0)
// rather than going through it.

// Backgrounds (darkest to lightest).
pub const BG_APP: Hsla = Hsla { h: 226.0 / 360.0, s: 0.15, l: 0.11, a: 1.0 }; // #1a1c24
pub const BG_PANEL: Hsla = Hsla { h: 222.0 / 360.0, s: 0.13, l: 0.14, a: 1.0 }; // #22252e
pub const BG_PILL: Hsla = Hsla { h: 230.0 / 360.0, s: 0.12, l: 0.25, a: 0.82 }; // rgba(56,60,74,0.82)
pub const BG_PILL_HOVER: Hsla = Hsla { h: 228.0 / 360.0, s: 0.06, l: 0.52, a: 0.8 }; // rgba(124,129,140,0.8)
pub const BG_PILL_ACTIVE: Hsla = Hsla { h: 228.0 / 360.0, s: 0.10, l: 0.35, a: 0.85 }; // rgba(78,82,99,0.85)

// Focus / accent.
pub const ACCENT: Hsla = Hsla { h: 201.0 / 360.0, s: 0.90, l: 0.75, a: 1.0 }; // #7fc8ff
pub const ACCENT_DIM: Hsla = Hsla { h: 201.0 / 360.0, s: 0.56, l: 0.52, a: 1.0 }; // #4a9bd4
pub const ACCENT_TEXT: Hsla = Hsla { h: 204.0 / 360.0, s: 0.45, l: 0.08, a: 1.0 }; // #0a1620

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

// Toolbar layout (per Allodium-tauri's three-zone convention).
pub const TOOLBAR_ROW1_H: Pixels = px(50.0);
pub const TOOLBAR_ROW2_H: Pixels = px(26.0);
pub const LEFT_PANEL_W: Pixels = px(220.0);
pub const CHANNEL_RACK_H: Pixels = px(260.0);
pub const MIXER_H: Pixels = px(190.0);

// Typography. JetBrains Mono is the desktop's own bar font; fall back to
// plain "JetBrains Mono" if the Nerd Font variant isn't resolvable by GPUI's
// text system. Revisit (per 01-theme.md) whether full-monospace UI reads
// well once real labels exist, vs. monospace-only for numeric displays.
pub const FONT_UI: &str = "JetBrains Mono";
