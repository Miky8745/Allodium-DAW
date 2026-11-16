use gpui::{Div, ParentElement, Styled, div, px};

use crate::theme;

/// A small caption under a header control — Allodium-tauri's own
/// `.dial-label` (SWING / TIME SIG / LEN).
pub fn group_label(text: &'static str) -> Div {
    div().text_size(px(7.0)).text_color(theme::BTN_TEXT.alpha(0.6)).child(text)
}
