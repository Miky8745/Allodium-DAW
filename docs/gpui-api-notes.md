# GPUI API notes

Carried forward from `Allodium-GPUI-test`'s prototype work (confirmed against
`gpui = "0.2.2"` from crates.io). Read this before guessing at GPUI API
shape — most of it was already worked out once.

Primary source for anything not covered here: the crate's own bundled
examples, read locally rather than cloning the whole Zed repo:

```
GP=$(find ~/.cargo/registry/src -maxdepth 2 -iname "gpui-0.2*")
ls $GP/examples   # uniform_list.rs, scrollable.rs, painting.rs, pattern.rs,
                  # drag_drop.rs, grid_layout.rs, animation.rs, etc.
ls $GP/src        # element.rs (the Element trait), interactive.rs, window.rs
```

## Entrypoint

```rust
Application::new().run(|cx: &mut App| {
    let bounds = Bounds::centered(None, size(px(W), px(H)), cx);
    let window = cx.open_window(
        WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), focus: true, ..Default::default() },
        |window, cx| cx.new(|cx| MyView::new(window, cx)),
    ).unwrap();
    cx.on_window_closed(|cx| cx.quit()).detach();
    cx.activate(true);
});
```

**Wayland/niri + NVIDIA swapchain bug** (confirmed on this machine, not
specific to the old WebKitGTK problem — a separate GPUI/Vulkan issue): the
swapchain isn't (re)created until the window's content size actually
changes, so nothing presents on screen until the first resize. Workaround
applied in `src/main.rs`: schedule a 1px resize-and-back ~200ms after
launch. See that file for the actual code — don't rediscover this.

## Reactive views

- View structs are wrapped in `Entity<T>` via `cx.new(|cx| ...)`, implement
  `Render`, and must call `cx.notify()` after mutating state that should
  trigger a re-render — not automatic/fine-grained.
- Per-frame update (anything that needs to animate every frame, e.g. a
  playhead sweep): call `window.request_animation_frame()` unconditionally
  at the top of `render()`, paired with `cx.notify()`. No `Animation`/
  `with_animation` API needed for this — that's for bounded, eased
  transitions on a single element, not an open-ended wall-clock-driven
  sweep.

## Painting: `canvas()` + plain `div()`, not a hand-rolled `Element`

Decision already made and validated in the prototype (`src/grid.rs`): use
GPUI's `canvas(prepaint_fn, paint_fn)` for all custom painting (direct
`paint_quad`/`paint_path`/text calls), wrapped in a plain `div()` that owns
the mouse/scroll handlers for input. This avoids hand-rolling hit-testing
(which a full `Element` impl would require) while still allowing fully
custom, hand-virtualized rendering.

- `uniform_list(id, item_count, cx.processor(|this, visible_range, window, cx| -> Vec<Item>))`
  exists and gives free *vertical-only* virtualization + scroll handling —
  considered but not used; the piano roll needs both-axis virtualization
  (notes span the horizontal step axis too), so it's fully hand-built
  inside the `canvas` paint closure instead (`first_row/last_row` +
  `first_step/last_step` computed from viewport bounds + scroll offset).
- Plain `div()`-based elements do **not** receive their own screen bounds in
  event handlers — only `canvas`'s `prepaint`/`paint` closures get a
  `bounds` param. Not an issue when the input `div` fills the whole
  window/pane with no offset: `event.position` is already in
  window-content coordinates, so grid-pixel coords are just
  `position + scroll_offset`.
- Mouse/scroll handlers on `div()`:
  `.on_mouse_down(button, cx.listener(|this, ev: &MouseDownEvent, window, cx| ..))`,
  `.on_mouse_move(cx.listener(...))`, `.on_mouse_up(button, cx.listener(...))`,
  `.on_mouse_up_out(button, cx.listener(...))` (fires on release *outside*
  the element — use this to cleanly cancel an in-progress drag),
  `.on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, window, cx| ..))`
  — `ev.delta.pixel_delta(line_height)` normalizes both `Lines`- and
  `Pixels`-flavored scroll deltas to one `Point<Pixels>`.
- `cx.listener(...)` is the pattern for closures needing `&mut self` on the
  view.

## Audio/UI clock sync

No GPUI involvement needed. Compute a shared playback position from
`samples_played / sample_rate` **inside the cpal audio callback itself**,
not from a separately-sampled `Instant` — sampling `Instant::elapsed()`
per-callback double-counts elapsed time against `samples_played` and makes
playback drift/speed up over a run. A UI-side `render()` can independently
derive the same position from a shared start `Instant` (passed by value,
`Copy`, no atomics/channels needed) and `elapsed_since_shared_start * rate`.
See `Allodium-tauri/docs/audio-playback.md` for the actual scheduling
behavior to replicate (pitch, ADSR, step-sequencer timing) once Stage 3
starts — this note is only about *how to keep a clock consistent*, not what
the DAW's scheduler needs to do.

## Interaction/layout gotchas

- **`.absolute()` escalates to the nearest `.relative()` *ancestor*, not
  the nearest sibling or parent `div()`.** A plain `.absolute().size_full()`
  overlay meant to cover one small sibling element (e.g. a click-catcher
  over a scroll-field readout) will instead cover whatever large
  positioned ancestor it actually finds — in one real case, an entire
  Channel Rack row, silently eating clicks meant for the step grid
  beneath it. Always wrap the overlay *and* the element it's meant to
  cover in their own `div().relative()` so `size_full()` resolves against
  that pair, not whatever happens to be positioned further up the tree.
- **A `MouseMoveEvent.pressed_button` is ambient, not scoped to "this
  drag."** Any element's `on_mouse_move` fires whenever the pointer
  passes over it with a button held, regardless of *why* that button is
  down — including an unrelated drag elsewhere (e.g. resizing a panel)
  that happens to carry the pointer across a grid of per-cell click
  targets below it. Checking `ev.pressed_button` alone cannot distinguish
  "the user is dragging over me to paint" from "the user is dragging
  something else and incidentally passed over me." For a drag-to-paint
  gesture (e.g. click-and-drag across step-sequencer cells), track an
  explicit boolean/enum state set only by the gesture's own `mouse_down`
  and cleared by a window/panel-level `on_mouse_up`/`on_mouse_up_out`,
  and gate the per-cell `on_mouse_move` handler on that state instead of
  on `pressed_button` alone.
- **This app funnels all keyboard handling through one `FocusHandle` +
  one `on_key_down` at `AppShell`'s own root** (`shell.rs`'s
  `focus_handle` + `.track_focus(&self.focus_handle).on_key_down(cx.
  listener(Self::on_qwerty_key_down))`), not a `FocusHandle` per panel.
  A panel that needs its own keyboard shortcuts (e.g. the Piano Roll's
  Delete/Backspace/Escape acting on a note selection) should add a
  routing check inside that one existing dispatcher — gated on whatever
  makes that panel "active" right now (open + frontmost, in the Piano
  Roll's case, via `AppShell::route_piano_roll_key`) — rather than
  giving the panel's own entity a second `FocusHandle`/`track_focus`.
  Two competing focus handles would fight over which one actually has
  focus, and clicking into the second panel would silently break
  whatever the first handle's own shortcuts were (here, QWERTY-typing-
  to-piano) the next time that panel is interacted with.
- **`f32`/`f64::clamp(min, max)` panics if `min > max` — never pass a
  size tracked from layout (a viewport width, a measured element size)
  as the *upper* bound of a clamp whose lower bound is a fixed minimum.**
  Real crash hit in practice: a scrollbar/minimap thumb width clamped to
  `.clamp(MIN_THUMB_W, viewport_w)` panicked the instant `viewport_w`
  was narrower than `MIN_THUMB_W` — which a tracked viewport size
  legitimately *is* for the first few frames (it starts at `px(0.0)`
  before a `canvas`'s paint closure ever reports a real size, same gap
  `grid_origin`/`grid_size`'s own doc comment calls out), and could also
  just happen in a genuinely narrow window. Fix: clamp the *bound*
  itself first (`let max = viewport_w.max(MIN_THUMB_W);`) so the two
  arguments to `.clamp()` can never cross, rather than assuming the
  tracked size will always be "big enough."

## Splitting a large view into `foo/*.rs` submodules

Not GPUI-specific, but came up splitting both `channel_rack.rs` and
`piano_roll.rs` this way, and is easy to get wrong: Rust privacy only
flows from an ancestor module down to its descendants, never sideways
between sibling submodules, and never back up from a child to its
parent. Concretely, for an entity type (e.g. `PianoRoll`) split into
`piano_roll.rs` (the struct + top-level `render()`) plus `piano_roll/
{a,b,c}.rs` (each its own `impl PianoRoll { ... }` block):
- A method `render()` calls directly must be `pub(super)` in whichever
  submodule defines it — a plain private `fn` is invisible to the
  *parent*, even though the parent is the one holding the `Entity`.
- A type used as one of `PianoRoll`'s own struct field types (e.g. a
  per-submodule drag-state struct) must also be `pub(super)` in its
  defining submodule, then `use`'d (even privately) in the parent — a
  private `use` still counts as a visible item to *that module's own
  descendants*, so other submodules can reach it via `super::TypeName`
  without needing their own direct import.
- Submodule **A** can call a `pub(super)` method/type defined in
  sibling submodule **B** by going through the shared parent
  (`super::b::thing`) — `pub(super)` visibility extends to the parent
  module *and all of its descendants*, which includes every sibling, not
  just the immediate parent.
- Small pure-math helpers used by *several* submodules (pixel/grid
  conversions, shared constants) are simplest left directly in the
  parent file rather than picking one submodule to own them and making
  every other one reach across — descendants already see all of a
  parent's private items for free, no `pub` needed in that direction.

## Performance lessons

- **Never put `eprintln!`/`println!` inside a per-event input handler**
  (`on_scroll_wheel`, `on_mouse_move`, etc.). Confirmed to cause a severe,
  misleading frame-rate regression under rapid input bursts that has
  nothing to do with GPUI's own rendering cost. If input needs debugging,
  log to an in-memory ring buffer and dump it on demand instead.
- `[profile.dev] opt-level = 1` + `[profile.dev.package."*"] opt-level = 3`
  (Zed's own recommended dev profile) is required or GPUI's own rendering
  is painfully slow even in dev builds; your own crate's code doesn't need
  the optimization, hence the split. Already set in this repo's
  `Cargo.toml`.
- No public fps/frame-time API from GPUI itself — hand-roll it (see
  `src/perf.rs`, ported from the prototype: rolling min/avg/max frame time
  logged to stderr every N frames).

## Build environment (NixOS) — already applied in this repo, don't redo

- `shell.nix` at the repo root provides `pkg-config` + native deps
  (alsa-lib, fontconfig, freetype, libxkbcommon, wayland, vulkan-loader,
  X11 libs, dbus, zlib, openssl) and sets `LD_LIBRARY_PATH` for runtime
  linking. **Always build/run via `nix-shell --run "cargo ..."`**, not bare
  `cargo` — pkg-config/linking fails otherwise even with `nix-ld` enabled
  system-wide.
- `gpui` 0.2.2 pulls in `zed-async-tar` → `xattr` 0.2.3, which references
  `libc::ENOATTR` unconditionally on Linux — current `libc` crate versions
  removed that (correctly; Linux has no `ENOATTR`, only `ENODATA` — the old
  alias was a bug in old `libc`). This is a real upstream incompatibility,
  not NixOS-specific — it would hit on any OS with a current `libc`. Fixed
  by vendoring a patched copy at `vendor/xattr-0.2.3/` (the Linux/Android
  branch uses `libc::ENODATA`; macOS/BSD keep the real `ENOATTR`) plus
  `[patch.crates-io] xattr = { path = "vendor/xattr-0.2.3" }` in
  `Cargo.toml`. **Don't `cargo update` or touch this patch** — it'll
  silently break the build again if the vendored copy drifts or gets
  dropped.
- Clean build from scratch takes several minutes (gpui pulls in a large
  transitive dep tree — blade-graphics, cosmic-text, taffy, resvg/usvg,
  plus some Zed-specific forks like `zed-reqwest`/`zed-async-tar`/
  `zed-xim`). Incremental rebuilds are much faster.
