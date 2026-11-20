//! Small, app-agnostic UI atoms shared across panels. None of these know
//! which view owns them — every callback they take is already a fully
//! built `impl Fn(&Event, &mut Window, &mut App)` (typically a
//! `cx.listener(...)` from the caller's own `Context<T>`), so a widget
//! here never needs to be generic over, or even aware of, the entity
//! type using it. That's what makes it reusable: a new panel can pull one
//! of these in without inheriting anything about Channel Rack, Mixer, or
//! whatever else already uses it.
//!
//! Most of these return a plain `Div` so a caller can keep chaining
//! builder methods (an extra `.on_hover(...)`, a `.shadow(...)`) onto the
//! result instead of needing its own near-identical copy just to add one
//! thing — see `toolbar::toggle`'s own use of `icon_toggle` for exactly
//! that. `dial` is the one exception: its own right-click "reset" menu
//! wraps the div in `ContextMenu<Div>`, so it returns `impl IntoElement`
//! instead.

mod dial;
mod group_label;
mod icon_toggle;
mod scroll_field;

pub use dial::dial;
pub use group_label::group_label;
pub use icon_toggle::icon_toggle;
pub use scroll_field::scroll_field;
