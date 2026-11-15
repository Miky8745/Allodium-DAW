# Toolbar: porting Allodium-tauri's full `Toolbar.svelte` feature set

Status: done, at the "compiles, runs, holds 60fps" level of verification
— same caveat as every other UI stage: nothing requiring a mouse,
keyboard, or eyes has been confirmed yet.

## The ask

Port every control in Allodium-tauri's `Toolbar.svelte` into this app's
toolbar — icons/labels/positions were free to change, but every original
control had to land somewhere. See `src/toolbar.rs` for the full
rundown of which original control maps to which new one and why. The
toolbar went from the single row Stage 8 left it at (`docs/stage-08-
visual-redesign.md`, which deliberately dropped a second, always-empty
row) back to two rows — row 2 is no longer empty, so it earns its place
back.

## Wiring fidelity, by category

**Fully wired to this rewrite's real backend**: Pattern/Song mode
switch, Play/Stop, tempo (new `ChannelRack::tempo`/`set_tempo`, scroll-
to-adjust with shift for ×10 steps, exactly mirroring the original's
`onTempoWheel`), step position (shown as a raw step count rather than
the original's beat:step:tick clock — this rewrite's grid is step-based,
not a musical-time clock), the four panel-visibility toggles, and
pattern selection (a cycle-button here instead of a `<select>`, matching
this codebase's own established "no dropdown for a handful of options"
convention from `cycle_channel_route` in `channel_rack.rs`).

**Ported at the same fidelity as the original itself**: several of the
original's own toggles — Record, Precount, Loop record, Auto-scroll,
One-click record, Snapping — are themselves nothing more than local
`$state` booleans in `Toolbar.svelte`, with no further consumption shown
in that file. Porting them as real, persistent, click-to-toggle controls
(stored in the new `ToolbarState`) matches that exact scope; none of
them drive a scheduler or audio feature, because the original's own
toolbar-level implementation didn't either. Pause reuses the real
play/stop pair (toggling it stops playback and remembers "paused" for
the next click to resume) — the one real gap against the original is
that resuming restarts from step 0 rather than the exact position,
since the audio engine has no resumable-position concept. Metronome
goes one step further and ports the original's one real side effect
(turning it on auto-starts playback if nothing's playing) — still no
audible click, since no metronome-tick scheduling exists in the audio
engine.

**Genuinely inert, no faithful equivalent possible**: the MIDI LED (no
MIDI crate/I/O anywhere in this codebase) and the Explorer panel toggle
(no file-browser panel exists yet — MVP item #8 in `daw-rewrite-starter/
02-feature-scope.md`, still unbuilt; `PanelKind` has no variant for it).
Both render dim and non-clickable, with hover text that says so plainly
rather than implying they work.

## One new, deliberately scoped feature: the hover-info readout

The original's row-2 info box shows the name/value of whatever control
was last hovered, fed by a `hoverInfoAction` used throughout the whole
app (every Dial/Knob/Slider, not just the Toolbar). Retrofitting that
everywhere is a separate, much bigger change. What's here instead:
`ToolbarState::hover` + the `toggle`/`inert` helpers' own `.on_hover(...)`
wiring, scoped to just this toolbar's own controls. Real and working,
just narrower in scope than the original.

## Architecture

`src/toolbar.rs` holds `ToolbarState` (the toolbar-only UI state: hover
info, live `Modifiers`, and the locally-stubbed toggle booleans) as a
field on `AppShell`, plus `AppShell::render_toolbar` and two small
builder helpers (`toggle`, `inert`) that every row-1/row-2 control goes
through. `Modifiers` (Shift/Alt/Ctrl) piggybacks on the workspace's
existing `on_mouse_move` handler in `shell.rs` rather than adding
separate `on_key_down`/`on_key_up`/`on_modifiers_changed` listeners —
one caveat worth knowing: the badges only refresh on mouse movement
inside the workspace, not on a pure keyboard-only modifier change with
the mouse held still.

## Not done here

No real audio-engine additions (metronome click, recording, MIDI,
oscilloscope/peak metering) — this pass is toolbar UI wiring onto
whatever the engine already exposes, not new audio-engine scope. Actual
grid snapping in the Piano Roll/Playlist isn't implemented either; the
Snapping control is a real, working toggle, it just has no grid to
affect yet.
