# Stage 5: Piano Roll, what was built

Status: done, at the "compiles, runs, holds 60fps" level of
verification — same honest caveat as Stage 4: nothing requiring a mouse,
keyboard, or ears has been confirmed yet.

**Note (later pass):** by the time the toolbar/Note Properties/
scrolling-nav follow-ups landed, `src/piano_roll.rs` had grown past this
stage's own single-file shape — it's now split into `src/piano_roll/
{interaction,toolbar,properties,scroll,paint}.rs`, the same way
`channel_rack.rs` was split into `src/channel_rack/*.rs`. The file
references below (`src/piano_roll.rs`) describe this stage's own
original contents; see that module split (and `piano_roll.rs`'s own
current top doc comment) for where each piece actually lives now.

## What this stage actually was: integration, not new design

As the stage brief predicted, the hard part was already solved.
`src/piano_roll.rs` ports `Allodium-GPUI-test/src/grid.rs` and
`src/model.rs` essentially wholesale — the virtualized row/column
striping, note painting, click-place/drag-move/right-click-drag-delete
with its fade animation, and scroll clamping are the same logic, just
retyped against the real project model's `Note` (`id: u32`,
`pitch: i32`, `start: f64`, `length: f64`, `velocity: f64`) instead of
the prototype's `(u64, u32, u32, u32)` synthetic shape. The real work was
wiring it to live state instead of a seeded `Vec<Note>`.

## How the Piano Roll shares state with the Channel Rack

`ChannelRack` already owned the `ProjectFile`, the sample cache, and the
`push_snapshot`/`save` plumbing from Stage 4. Rather than duplicate any
of that or hoist project ownership up into a new shared model, the Piano
Roll just holds an `Entity<ChannelRack>` and talks to it:

- `ChannelRack::selected_channel_id` is new state, set by clicking a
  channel's name in the rack (which already auditioned the sample —
  clicking now also selects it). This satisfies the stage brief's "a way
  to pick which channel's notes the grid is showing" with the brief's own
  suggested minimal approach.
- `ChannelRack::notes_for`/`update_notes`/`persist`/`pattern_length_steps`/
  `channels`/`is_playing`/`current_step` are the new read/write surface
  the Piano Roll needs; all delegate to state `ChannelRack` already had.
- The Piano Roll keeps its own local `notes: Vec<Note>` as the live
  editing buffer (reloaded from `notes_for` only when
  `selected_channel_id` changes, never clobbering an in-progress drag),
  mirroring exactly how the prototype's `grid.rs` held its own `Vec<Note>`
  — just synced to the shared model instead of being the only copy.

## The live-edit / disk-save split, reused from the slider pattern

Stage 4's volume/pan slider already established the right split for a
continuous drag: push a fresh snapshot to the audio engine on every
`mousemove` (so the edit is audible immediately), but only write to disk
on `mouseup`. Piano Roll note dragging is the same shape of problem (a
multi-frame gesture, not a discrete click), so it reuses the identical
split: `commit_live` (snapshot + notify, every mutation) during
place/move/delete, `persist` (actual `save_project` disk write) only in
`end_drag`. This avoids a disk write per mouse-move frame during a drag,
which the stress test below would have otherwise stressed for no reason.

## Playhead and pattern length come from the engine, not a new clock

The prototype drove its playhead off a standalone wall-clock sweep
(`Instant::now()` plus a fixed `STEPS_PER_SEC`) because it had no real
scheduler to ask. This stage has one: `ChannelRack::current_step()`
already exposes the Stage 3/4 `AudioEngine`'s sample-accurate position
(`current_step` is `-1.0` when not playing, so the playhead is simply
not drawn rather than needing a separate visibility flag). Likewise, grid
width no longer uses the prototype's fixed `PATTERN_LEN = 256` constant —
`ChannelRack::pattern_length_steps()` derives it from the selected
pattern's `length_beats`, the same formula `build_snapshot` already uses
for the scheduler, so the grid and the actual loop point can never
disagree.

## What's explicitly not here, per the stage's own scope cut

Zoom, marquee multi-select, copy/paste, note properties beyond velocity,
auto-extend pattern length, slide notes — all deferred per
`02-feature-scope.md`, matching `stages/05-piano-roll.md`'s own list.
Velocity is present only as the data field (`DEFAULT_VELOCITY = 1.0` on
a freshly placed note); there is no velocity-lane UI.

## Verification — and its real limits

`cargo check` and `cargo test` are both clean (9 tests, all passing,
unchanged from Stage 4 — this stage added no new unit-testable logic,
just UI wiring). Ran the actual app: it opens with the Channel Rack on
top and the Piano Roll below per the new two-row workspace split in
`shell.rs`, holds a steady ~60fps per `perf.rs` (16.6-17.1ms avg across
several 120-frame windows), no panics, no `cpal` stream errors.

**Not verified, same limits as Stage 4 and still accumulating**: this
environment cannot drive a native window's mouse/keyboard or play audio
to a listener. Note placement, dragging, right-click-drag deletion, the
channel-selector click-through from the rack, the playhead actually
tracking real playback, and whether a placed note is audible at the
right pitch/time are all unverified beyond "the code path exists and the
view renders without crashing." The stage brief's own stress test
(scroll + drag + delete while audio plays, watching for steady fps) has
only been run passively (idle render, no input) — a real interactive
pass, plus Stage 3's still-outstanding listening pass, remain owed
together before trusting this stage's behavior.
