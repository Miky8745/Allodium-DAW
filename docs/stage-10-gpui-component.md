# Adopting gpui-component for styled widgets

Status: dependency swap done, build verification in progress. Triggered
by the user's feedback that the hand-styled UI (Stage 8/9's manual
`div()` buttons/pills/panels) still read as "an ImGUI app" — flat,
static, no real hover/press feedback despite Stage 9's attempt to add
some by hand. Rather than keep hand-rolling widget polish file by file,
switched to a real component library.

## The compatibility question, and how it resolved

The obvious worry: our `Cargo.toml` depends on the crate literally
published as `gpui` v0.2.2, but `gpui-component` (the styled widget
library we want) depends on a *different*, separately-published crate —
`gpui-pre` v0.3.12 — renamed to `gpui` only inside its own source via
Cargo's `package = "..."` rename feature. These looked like two
unrelated crates that couldn't coexist.

They turned out to be the same lineage at two different snapshot points,
not a fork. A direct source diff of everything this codebase actually
calls — `Modifiers`, `MouseMoveEvent`, `ScrollWheelEvent`, `KeyDownEvent`,
`.hover()`, `.active()`, `on_scroll_wheel`, `on_hover`, and the top-level
exports (`Entity`, `Context`, `Render`, `canvas`, `px`, `point`, `Bounds`,
`WindowOptions`, `Application`, `SharedString`, …) — found every
signature identical between `gpui-pre` 0.3.12 and our `gpui` 0.2.2,
differing only by added modules (accessibility, profiler, queue,
gestures). `gpui-pre` is simply a later publish of the same upstream
GPUI snapshot lineage.

The maintainers also publish an umbrella crate, `gpui-kit`, whose
`lib.rs` does `pub use ::gpui::*` — a full re-export of the real GPUI
namespace — plus `gpui_kit::component::*` (gpui-component) and
`gpui_kit::base::*` (gpui-base, which the component layer sits on).
Their own doc comment explicitly endorses consuming it under the name
`gpui` via Cargo's rename feature, specifically so existing code's
`gpui::` paths keep resolving with zero changes. That's what this repo
now does:

```toml
gpui = { package = "gpui-kit", version = "0.9.1" }
```

Every pre-existing `use gpui::{...}` in every file in this codebase is
unchanged — they all still resolve, now through the richer re-export.
Nothing needed renaming. New code can additionally reach the styled
widgets at `gpui::component::button::Button` etc., and `gpui::base::Root`
for the window-root wrapper below.

## What changed in `main.rs`

Two additions, nothing removed:

- `gpui::init(cx)` once, before opening any window — required by
  gpui-component (it also initializes gpui-base).
- The root view is now wrapped: `cx.new(|cx| gpui::base::Root::new(app_shell.into(), window, cx))`
  instead of returning the bare `AppShell` entity from `cx.open_window`'s
  build closure. `Root::new` takes `impl Into<AnyView>`, so it's type-erased
  — no generic plumbing needed elsewhere. This is exactly what
  `gpui_kit::open_window`'s own source does internally; done by hand here
  instead so the existing Wayland/NVIDIA swapchain-nudge workaround
  (which only touches the returned `WindowHandle` and `cx`, not the view
  type) didn't need touching.

## One real runtime gap found by actually running it

The build succeeded on the first real attempt, but the app came up with
a visibly blank/frozen window — `cargo test`'s 15 tests still passed and
`cargo build` was clean, neither would have caught this. `perf.rs`'s
frame-time logging (confirms render ticks are actually happening) never
printed a single line even after 80+ seconds idle, but manually
resizing the window via the compositor (`niri msg action
set-window-width`) made it immediately start rendering at a normal
frame rate. That's the exact symptom the existing Wayland/NVIDIA
swapchain-presentation workaround in `main.rs` exists for (see the
comment there) — except the workaround itself, unchanged from before
this migration, wasn't fixing it anymore.

The fix: the workaround used to do a grow-resize and a shrink-resize
back-to-back inside one synchronous closure. Under gpui-component's
Linux platform backend, calling `window.resize()` twice with no real
time between them gets coalesced away before it reaches the compositor
as an actual resize event — so the swapchain never gets recreated.
Splitting it into two separate `await` steps (timer, grow, timer,
shrink) made it work again. Confirmed via temporary `eprintln!`s that
both resize calls were in fact executing successfully (`Ok`) even while
rendering stayed frozen — the calls weren't failing, they just weren't
having the intended platform-level effect without the gap between them.

**Takeaway for future dependency bumps in this repo**: a clean
build + passing test suite does not verify the window actually renders
continuously on this machine. Always do a real `cargo run` + idle-wait
check after any GPUI version change, the way Stage 8's writeup already
cautioned for UI changes generally.

## Not done yet

No existing hand-built widget has been swapped over to a gpui-component
equivalent yet — this pass is the dependency/bootstrap plumbing only.
Picking which buttons/pills become real `gpui::component` widgets (and
which stay custom, like the Piano Roll/Playlist/Mixer's canvas-painted
grids and drag-based sliders, which have no component-library
equivalent anyway) is the next pass.
