# Stage 2: Data model and persistence, what was built

Status: done. `src/project.rs` defines the shared project data model and
XML save/load; verified end to end across separate process launches, not
just unit tests.

## Design choice: multi-pattern from the start

The stage brief allowed simplifying to a single pattern for the first cut
and generalizing later, or mirroring Allodium-tauri's "channels shared
across patterns, content stashed per pattern" model up front. Went with
the latter: `PatternContentEntry` holds one channel's steps/notes for one
specific pattern (keyed by `pattern_id` + `channel_id`), and `Channel`
itself holds no step/note data directly. Reasoning: the Playlist stage
needs multiple patterns arranged on a timeline regardless, and the
original's model is already proven, so a simpler interim single-pattern
model would just need generalizing again in three stages for no real
savings now.

## What's in the model (MVP scope only)

- `Channel` — id, name, sample path, mute/solo, pan, volume, mixer track
  routing. Single-sample only; no multisample folders.
- `Note` — id, pitch, start, length, velocity. No pan/release/mod-X/
  mod-Y/fine-pitch/color/slide (all stretch-scope Piano Roll properties).
- `PatternContentEntry` — a channel's steps (`Vec<bool>`) and notes for
  one pattern.
- `Pattern` — id, name, length in beats. No color (pattern recolor is
  stretch).
- `Placement` — a pattern block on a Playlist track: id, pattern id,
  track id, start beat. No `kind` sentinel for clip placements since
  those (audio-file blocks, as opposed to pattern blocks) are entirely
  deferred for the MVP — only one kind of placement exists right now.
- `MixerTrack` — id, volume, pan, mute/solo, `routes_to` (sentinel `0` =
  routed straight to Master). No insert-effect rack.
- `ProjectFile` — the root: beats per bar, step count, tempo, selected
  pattern, and the five lists above, each wrapped in its own `*Xml`
  struct (quick-xml repeats a bare `Vec<T>` field's own tag name per
  element rather than emitting one wrapper tag per list, so each list
  gets a small wrapper with a renamed inner field — same pattern as the
  original).

## Persistence

Plain functions (`save_project`, `load_project`, `projects_root`), not
commands — a native app has no IPC boundary to route them through, which
is genuinely simpler than the original, not just differently shaped.
Projects live at `~/.local/share/allodium/projects/<name>/<name>.alp`,
same quick-xml-based XML approach as the original (pretty-printed, 2-space
indent), but a fresh format for this new model rather than the original
`.alp` schema — no need for byte compatibility with the Tauri app's files.

Conventions kept from the original, and why (see `project.rs`'s own
module doc comment for the short version):

- String "none" fields are `""`, not `Option<String>` — quick-xml
  round-trips `Option::None` as an empty tag that deserializes back as
  `Some("")`, not `None`.
- Numeric "none" fields use a sentinel (`0`) for the same reason, since an
  empty tag's content isn't valid text for a numeric `Option` to parse at
  all.
- Every field is `#[serde(default)]` so files saved by an earlier version
  of the model keep loading once new fields get added later (verified
  with a test using a hand-written "old" XML file missing `mixerTracks`
  and `stepCount` entirely).

## Verifying "done" actually meant running the app twice

The stage's own bar was "verified by actually closing and reopening the
app, not just by reading the code" — so beyond the two unit tests in
`project.rs`, `main.rs` currently runs a `verify_persistence()` check on
startup that logs to stderr: load the `"default"` project, and if none
exists, create one with a single test channel and save it. Confirmed
across three separate `cargo run` invocations: the first (after clearing
`~/.local/share/allodium/projects`) created the project; a later,
genuinely separate process launch loaded that exact file back and printed
the same channel (`"Test Channel"`, volume `0.8`, pan `0.5`). This
stderr-log check is scaffolding, not a permanent feature — it comes out
once Stage 3 or a later stage gives project creation a real UI.
