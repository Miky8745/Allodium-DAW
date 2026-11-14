# Stage 1: Foundation — what was built

Status: scaffold complete, three-zone empty shell renders, confirmed
building/running via `nix-shell --run "cargo run"`.

## Scaffold

- `Cargo.toml`: package `allodium`, deps `gpui = "0.2.2"`, `cpal = "0.15"`,
  the Zed dev-profile split (`opt-level = 1` own crate / `3` dependencies),
  and the `xattr` crates-io patch — see `gpui-api-notes.md` for why each of
  these exists.
- `shell.nix` and `vendor/xattr-0.2.3/` copied verbatim from
  `Allodium-GPUI-test`.

## Modules

- `src/theme.rs` — design-token constants (`Hsla`/`Pixels`) translated
  from `daw-rewrite-starter/01-theme.md`'s derived values (backgrounds,
  accent blue `#7fc8ff`, radii, toolbar/left-panel dimensions, font). Every
  later stage should import from here rather than hardcoding a
  color/radius/font inline.
- `src/shell.rs` — `AppShell`, the top-level `Entity<AppShell>` /
  `Render` view. Builds the three-zone layout mirroring Allodium-tauri's own
  convention (toolbar row 1 `50px` + row 2 `26px`, left panel `220px`,
  workspace filling the rest) — empty `div`s with theme background colors
  only, no real toolbar/panel content yet (that's each later stage's job).
  Drives `window.request_animation_frame()` + `FrameStats::tick()` every
  render so frame timing is observable from Stage 1 onward.
- `src/perf.rs` — ported unchanged from the prototype (rolling frame-time
  logger).
- `src/main.rs` — entrypoint; window sized `1400x900` (a DAW needs more
  room than the prototype's `1100x700`); includes the Wayland/niri
  swapchain-presentation nudge workaround (see `gpui-api-notes.md`).

## Open items / notes for later stages

- The three-zone layout currently has no toolbar buttons, file explorer,
  or workspace content — that's Stages 3–8's job, not a Stage 1 gap.
- `FONT_UI` in `theme.rs` is set to `"JetBrains Mono"`; `01-theme.md` flags
  that full-monospace UI (not just numeric displays) should be
  sanity-checked once real labels exist — revisit when the toolbar/channel
  rack get real text content.
- Left-panel width (`220px`) and toolbar row heights were taken directly
  from Allodium-tauri's own pixel values; no redesign attempted here, per
  `01-theme.md`'s instruction to apply new colors/radii without
  re-deriving layout dimensions that were already fine.
