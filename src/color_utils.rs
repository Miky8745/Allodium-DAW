//! Small color-math helpers with no UI/entity state of their own — split
//! out of `channel_rack` (where "Recolor randomly" first needed them) so
//! any future feature needing packed-RGB color math doesn't have to
//! reach into that module for it.

use gpui::Hsla;

/// `(r*299 + g*587 + b*114) / 1000` — the exact luminance formula
/// Allodium-tauri's `ChannelRow.svelte` (`dropTextColor`) uses to pick
/// readable text over an arbitrary custom background color.
pub fn contrast_text_color(rgb: u32) -> Hsla {
    let r = ((rgb >> 16) & 0xFF) as f32;
    let g = ((rgb >> 8) & 0xFF) as f32;
    let b = (rgb & 0xFF) as f32;
    let luminance = (r * 299.0 + g * 587.0 + b * 114.0) / 1000.0;
    if luminance < 140.0 {
        Hsla { h: 0.0, s: 0.0, l: 0.92, a: 1.0 }
    } else {
        Hsla { h: 0.0, s: 0.0, l: 0.08, a: 1.0 }
    }
}

/// `h`/`s`/`l` each `0.0..=1.0` → packed `0xRRGGBB`.
pub fn hsl_to_rgb_u32(h: f32, s: f32, l: f32) -> u32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (((h * 6.0) % 2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = match (h * 6.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let r = ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u32;
    let g = ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u32;
    let b = ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u32;
    (r << 16) | (g << 8) | b
}
