# Visual + UX redesign: floating panels, what was built

Status: done, at the "compiles, runs, holds 60fps" level of
verification — same caveat as every prior UI stage: nothing requiring a
mouse, keyboard, or eyes has been confirmed yet. This isn't a numbered
pipeline stage; it's a dedicated visual/UX pass requested after Stage 7,
covering part of Stage 8's own scope (the floating-window decision and a
theme-consistency pass) but not its full validation/stress-test/final-
writeup scope, which is still outstanding.

## The brief, and the two corrections it started from

The user asked for: distinctive visuals that genuinely belong on their
own desktop, not a 1:1 port of Allodium-tauri's look, with existing
button positions kept roughly where they are, and the original's
"window-based" feel restored — Channel Rack/Piano Roll/Playlist/Mixer as
draggable panels again, not fixed stacked chrome.

Two research passes grounded every decision: a full audit of
Allodium-tauri's `app.css` + components, and a *deeper* re-audit of
`~/Programming/nixos_conf` than the one `01-theme.md` was originally
written from. That second pass found two real errors already baked into
`theme.rs`:

- **`FONT_UI` was wrong.** `"JetBrains Mono"` isn't actually installed
  on this machine (confirmed via the alacritty config's own comment
  noting it silently falls back) — the real, everywhere-used font is
  `"JetBrainsMono Nerd Font Mono"` (one word, Nerd Font *Mono* variant).
  Fixed.
- **`BG_PILL`'s alpha was `0.82`, not `0.8`** — a rounding slip from
  whichever summary originally produced it. Fixed against the live
  `waybar/style.nix` value.

It also surfaced that the desktop's actual wallpaper
(`~/Background/gargantua-black-...jpg`) is a vivid warm-amber-vs-cool-
cyan sci-fi scene, not the black/starfield image an earlier session had
assumed — which became the justification for this pass's one new color
(see `WARM_ACCENT` below) rather than an arbitrary addition.

## Color and type: one correction, one new rare accent, no new typeface

`BG_APP`/`BG_PANEL`/`ACCENT`/`ACCENT_DIM`/`BORDER` all kept their
existing values — all now re-verified against the live config rather
than resting on an earlier summary. Two additions to `theme.rs`:

- **`WARM_ACCENT`** formalizes a color that already existed as a magic
  number (`rgb(0xff6a3d)`, the Piano Roll's delete-flash animation) into
  a named constant whose reason is now explicit: a deliberate, rare nod
  to the wallpaper's own orange-vs-cyan contrast, not a second general-
  purpose accent. Its usage stayed exactly where it was (delete-flash
  animations in both the Piano Roll and the Playlist, which previously
  used two different ad-hoc reds/oranges — now the same one color) —
  restraint, not an excuse to spread a new hue around the app.
- **`pattern_color(pattern_id) -> Hsla`** — golden-ratio hue stepping
  gives every Playlist pattern a distinguishable, deterministic color
  with no new persisted field. Used for placed blocks and the pattern-
  selector pills, which previously all rendered in the same flat
  `ACCENT_DIM`.

No second typeface was introduced. The original's serif panel-title
font doesn't exist anywhere on this desktop (which is monospace end-to-
end: waybar, swaync, wlogout, alacritty all pin the same Nerd Font Mono)
— introducing one would have broken the "native to this machine" goal
the whole redesign is built on. Panel-title distinction instead comes
from a bold weight plus a plain Unicode glyph prefix (▦ ♪ ☰ ▬ for the
four panels, ▶/■ for Play/Stop) — deliberately *not* Nerd-Font-specific
PUA codepoints, which would have needed guessing exact glyph positions
with real tofu/mis-render risk; these are standard BMP symbols that
render correctly in any font while still giving the same icon-plus-
mono-label idiom waybar's own modules use.

## Panel chrome: departing from the original on purpose

No drop shadows, no 9px rounding at rest — both explicitly avoided as
the generic "SaaS card" tell (same soft shadow under every card,
regardless of focus). Instead, `src/floating_panel.rs` reuses *niri's
own* convention directly, since that's the window manager actually
running on this machine: `RADIUS_SM` (6px) corners, **no border at all
when unfocused**, and the focused panel gets a solid `ACCENT` ring
(finally consuming `FOCUS_RING`/`FOCUS_RING_WIDTH`, unused since Stage
1). A shadow (`shadow_lg()`) appears only while a panel is actively
being dragged, then flattens back to none at rest — the one deliberate
motion moment, matching this desktop's own near-total absence of hover/
transition animation (confirmed: `waybar/style.nix` sets
`transition: none` explicitly, everywhere).

**Shape language**: mute is now a circular dot, solo a square, in both
the Channel Rack and the Mixer (previously both were identical squares
differing only by fill color) — borrowed from the original, genuinely
useful since this app has only one accent color token so shape has to
carry the distinction color can't. The Channel Rack's step row also
gained 4-step beat-group shading to match the Piano Roll's own banding
— two grids that used to look unrelated now read as one system.

## Architecture: panels are plain Rust state, not a second GPUI window

`src/shell.rs`'s `AppShell` owns `PanelGeometries` (position/size/open
per panel, **not persisted** — matches the original's own documented
choice that window layout is session-only) and a single `z_order:
Vec<PanelKind>` whose last element is "frontmost," driving both render
order and the focus ring. None of the four panel views
(`ChannelRack`/`PianoRoll`/`Playlist`/`Mixer`) know they're floating —
`floating_panel::render_panel` just wraps whatever they render.

- **Drag/resize** reuse this codebase's own established pattern exactly
  (relative delta from mouse-down, move/up listeners on the outer
  workspace container rather than the small handle) — the same
  reasoning already used for the volume/pan sliders and the Piano Roll/
  Playlist note/placement drags. One `Option<PanelDrag>` on `AppShell`
  (kind: Move/Resize, which panel, start position/size) is the same
  shape as `ChannelRack`'s existing `SliderDrag`.
- **Focus-on-click-anywhere** was the one real technical unknown, and
  it's resolved by reading GPUI's own source (`div.rs`): a parent's
  `on_any_mouse_down` fires regardless of what a child's own mouse
  handler does (two-phase capture/bubble dispatch, not an exclusive
  single-target model) — so the panel wrapper raises+focuses on any
  click without interfering with the Piano Roll grid's or a slider's own
  handling underneath.
- **The toolbar collapsed from two rows to one** — the second row never
  had a purpose, and nothing in this pass needed it. It now holds app
  branding (left) and four toggle pills (right), one per panel,
  implementing the original's exact 3-state semantics: closed → open +
  focus; open-but-not-frontmost → raise to front (stays open); open-
  and-already-frontmost → close.
- **The dead 220px left sidebar placeholder was removed** — it never
  rendered anything, isn't part of the floating-window model (the
  original has no persistent sidebar in its own floating-window desktop
  either), and freed the full window width for the workspace.
- **Piano Roll's title is now dynamic** (`"Piano Roll — {channel}"`),
  computed by `AppShell` and passed into the shared chrome, instead of a
  second header div duplicated inside `piano_roll.rs` itself (removed).
- **Mixer's fader height now responds to its panel's actual (resizable)
  size** instead of a fixed constant. This turned out *simpler* than the
  problem the original had to solve for the same feature — the original
  needed real DOM measurement (`bind:clientHeight`) because the browser
  owned layout; here, `AppShell` already owns each panel's exact size as
  plain Rust state, so it just computes the available height and calls
  `Mixer::set_fader_height` directly, no measurement trick needed.

## What's explicitly not here

No custom app logo/icon asset (the original's leaf mark is its own
brand; a new one is a separate request). No maximize button — drag +
resize + close covers "window-based" without the original's full
feature set, a deliberate cut. No real-time level metering in the Mixer
(no metering engine hook exists; this pass restyles existing controls,
it doesn't build new audio-engine features). No file browser (unbuilt
MVP item #8, unrelated to this pass). No change to Stage 7's single-
target-routing scope cut.

## Verification — and its real limits

`cargo check` is clean; `cargo test` passes all 15 (unchanged — this
pass touches no scheduling/mixing logic, pure UI/layout). Ran the actual
app: it opens with all four panels in their initial roughly-stacked
layout, holds a steady ~60fps per `perf.rs` (16.7-17.4ms avg across
several 120-frame windows) with the new drag/resize/z-order machinery
active, no panics, no `cpal` stream errors.

**Not verified, same limits as every UI stage so far, now covering the
most interaction-heavy change yet**: this environment cannot drive a
real mouse/keyboard or view the rendered window. Dragging a panel by its
title bar, resizing from the corner handle, the toolbar's 3-state
toggle behavior, the focus ring actually appearing on the right panel,
and — most basically — whether any of this *looks* like a deliberate,
cohesive redesign rather than a plausible-sounding one are all
unverified beyond "compiles, runs, holds frame rate." A real interactive
pass is owed here more than anywhere so far, stacking on the audio
listening pass still outstanding since Stage 3.
