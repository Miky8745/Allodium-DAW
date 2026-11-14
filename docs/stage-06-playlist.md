# Stage 6: Playlist, what was built

Status: done, at the "compiles, runs, holds 60fps" level of verification
— same honest caveat as Stages 4/5: nothing requiring a mouse, keyboard,
or ears has been confirmed yet.

## What this stage needed that didn't exist yet: patterns

Every prior stage assumed a single pattern (`ProjectFile::default()`
creates exactly one, "Pattern 1"). A Playlist is meaningless with only
one pattern to place, so this stage had to add the pattern management
UI that `02-feature-scope.md` never called out as its own item — a
small pattern-selector bar (pills + "+ Pattern") now lives at the top of
`src/playlist.rs`, backed by new `ChannelRack` methods
(`patterns`/`selected_pattern_id`/`select_pattern`/`add_pattern`).
Clicking a pattern pill sets `ProjectFile::selected_pattern_id` — the
same field the Channel Rack and Piano Roll already read/write, so
there's exactly one "active pattern" concept shared by all three views
(mirrors the original's single `patternStore.selectedPatternId`), not a
separate Playlist-only notion of "which pattern gets drawn."

## Song-mode scheduling: the real new piece

`stages/06-playlist.md` is explicit that this is the one deferred piece
of Stage 3 that has to land here, since a Playlist is useless without
it. `src/audio/mod.rs` grew:

- `build_song_snapshot` — flattens every `Placement` into
  `SongPlacementVoiceData { start_step, length_steps, channels }`:
  `start_step = start_beat × 4`, and (the stage brief's explicitly
  invited simplification) `length_steps` is the placement's pattern's
  **nominal** `length_beats × 4`, not the original's content-driven
  "longest channel's actual reach, floored to the manual length."
  Noted here, and in `PROGRESS.md`, as the one deliberate scope cut —
  revisit if patterns start getting drawn with content that overflows
  their nominal length and the Playlist block doesn't reflect it.
- `trigger_channel_step` — the step/note-triggering logic that used to
  live inline in pattern mode's branch of the `cpal` callback, extracted
  so song mode can call the exact same logic (just against a
  placement-local step index) instead of a second hand-copied version
  that could silently drift from the first.
- The callback now branches on a new `song_mode: AtomicBool` (toggled
  independently of `playing`, so the PAT/SONG pill works whether the
  transport is running or stopped): song mode walks every active
  placement at the current absolute step and triggers its channels with
  a placement-local step index; once the absolute step passes the
  arrangement's length it flips `playing` to `false` itself, from inside
  the real-time callback — "stop at the end, don't loop," matching the
  original's documented Song-mode behavior, implemented as a plain
  atomic store rather than a callback/channel back to the UI thread.
- **Simplified, noted rather than built**: the original's `loopNotes`
  distinction (step channels tile to fill their block in song mode, note
  channels don't) isn't implemented — neither mode tiles a channel's
  steps past its own fixed 16-step row. This is the already-deferred
  "per-channel independent looping" from `02-feature-scope.md`'s stretch
  list, not a new gap this stage introduced; `trigger_channel_step`'s own
  doc comment calls this out at the source.
- 3 new unit tests (`audio::song_snapshot_tests`) cover the length-
  rounding math: an empty arrangement is one bar, a placement's steps
  come from its pattern (not a hand-set length), and the arrangement
  length rounds *up* to a whole bar rather than truncating.

## The Playlist view itself

`src/playlist.rs` is new design (no prototype exists for this one, per
`03-reuse-map.md`), but deliberately reuses two decisions already made
elsewhere in this codebase rather than inventing fresh ones:

- **Grid-origin capture for mouse math** — the exact same
  `Rc<Cell<Point<Pixels>>>` trick Stage 5's mouse-offset bugfix
  introduced, since this view has the same problem (a `canvas`-painted
  grid that doesn't fill the window, and plain `div()`s don't expose
  their own bounds to event handlers).
- **No-overlap placement, not snap-to-nearest-free-slot** — the stage
  brief's own explicitly-offered simplification over the original's
  `nearestFreeStart()`: `Playlist::overlaps` just rejects a placement
  draw/move that would overlap another block on the same track; the
  original's "politeness" of snapping to the nearest free slot instead
  is stretch scope, not built.
- **Placement blocks are plain `div()`s, not hand-painted** — unlike the
  Piano Roll's note painting, there are only ever a handful of
  placements on screen at once, so there's no virtualization problem to
  solve, and rendering them as real elements gets GPUI's text layout for
  pattern-name labels for free instead of hand-shaping text into a
  `canvas`. Row striping, bar gridlines, and the Song-mode playhead are
  still hand-painted (cheap, no text needed).
- **Delete-drag with a fade, duplicated not shared** — the Piano Roll's
  `DyingNote` idea ported as `DyingPlacement`, independently implemented
  rather than factored into a shared abstraction, per the stage brief's
  own explicit preference ("if it's cheap to share code... otherwise
  just duplicate the small amount of logic rather than building a shared
  abstraction prematurely").

8 fixed tracks, not persisted (`TRACK_COUNT`), mirroring the original's
own "local state, not a store" convention — there's no per-track
behavior yet beyond a visual lane. 64 bars of timeline rendered
(`BAR_COUNT`), one beat per grid cell (`BEAT_W`), matching the original's
documented grid resolution.

## What's explicitly not here

Clip (audio-file) placements, pattern rename/recolor, the overview/
minimap bar, marquee multi-select, Ctrl-drag-select — all per the stage
brief's own "explicitly not needed here" list and `02-feature-scope.md`.

## Verification — and its real limits

`cargo check` and `cargo test` are both clean (12 tests now — the 9
from before plus 3 new ones for `build_song_snapshot`'s length math).
Ran the actual app: it opens with all three views stacked (Channel Rack
/ Piano Roll / Playlist) under the toolbar, holds a steady ~60fps per
`perf.rs` (16.6-16.8ms avg across several 120-frame windows) with the
new views rendering, no panics, no `cpal` stream errors.

**Not verified, same limits as every prior UI stage and still
accumulating**: this environment cannot drive a native window's mouse/
keyboard or play audio to a listener. Placing/moving/deleting a pattern
block, the pattern selector, the PAT/SONG toggle's actual effect on
playback, whether song-mode playback plays the right patterns in the
right order/overlap and stops cleanly at the end, and whether any of it
is audible and correct are all unverified beyond "the code path exists,
compiles, and the view renders." The stage's own "done when" (place a
couple of patterns, hit Song-mode play, hear the right thing happen) is
a real interactive + listening pass still owed, stacking on top of
Stage 3's and Stage 5's own outstanding listening/interaction debt.
