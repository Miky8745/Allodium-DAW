# Stage 4: Channel Rack, what was built

Status: done, at the "compiles and doesn't crash" level of verification.
Nothing requiring a mouse, keyboard, or ears has been confirmed yet — see
the honest caveats at the bottom.

## Why the audio engine changed shape again

Stage 3 deliberately built `AudioEngine` around an immutable
`PlaybackSnapshot`, with a note that this was a simplification to revisit
"the moment Channel Rack actually wants to toggle a step while the
pattern is playing." That moment is this stage: the brief's own "done
when" says toggling a step, changing volume/pan, or mute/solo must
audibly affect playback *immediately*, and click-to-audition has to work
whether or not the transport is even running.

Two structural changes followed from that:

1. **The `cpal` stream is now persistent.** `AudioEngine::new()` opens
   the device and starts the stream once, for the engine's entire
   lifetime, instead of Stage 3's "build a fresh stream every `play()`."
   This is what lets a Preview one-shot play while stopped — there's
   always a live stream to push it into.
2. **Content arrives via a channel, not a closure capture.** `play()` no
   longer takes a snapshot; `set_content()` sends an `EngineMessage::
   UpdateContent` that the callback drains (non-blocking `try_iter()`,
   applied once per buffer — a few milliseconds of latency, imperceptible
   for a UI edit) and swaps into the local state it schedules from.
   `preview()` sends `EngineMessage::Preview`, consumed the same way, and
   those voices render every buffer regardless of the `playing` flag.

Measured side effect worth noting: `play`/`stop` are now *more* than
"cheap" (Stage 3's ~24-27ms measurement) — with no stream to rebuild at
all, they're just atomic stores, effectively free. Stage 3's own doc
still stands as the record of what that measurement looked like before
this stage's redesign; not rewritten here, just superseded in practice.

## Single-solo, ported exactly

Allodium-tauri's Channel Rack solo is deliberately **single-solo**:
soloing one channel clears every other channel's solo instead of
allowing several simultaneously. `toggle_solo` reproduces that (clear
everyone, then set only the target if it wasn't already soloed — toggling
an already-soloed channel just clears it). `build_snapshot` folds this
into each channel's effective mute: `ch.muted || (any_soloed &&
!ch.soloed)`, the same "explicit mute always wins regardless of solo"
rule the original documents under "Channel Rack solo."

## The slider: a relative drag, not an absolute read

GPUI's plain `div()` doesn't hand its mouse-event handlers the element's
own screen bounds — only a `canvas`'s prepaint/paint closures get a
`bounds` parameter (confirmed in `docs/gpui-api-notes.md` from the
prototype work). That makes "where inside this 64px-wide bar did the
user click" awkward to compute directly. The slider sidesteps it
entirely: `on_mouse_down` records the window-content x-position and the
control's *current* value; drag updates compute `delta = (current_x -
start_x) / SLIDER_W` and clamp `start_value + delta` to `[0, 1]` — no
absolute position against the element's bounds ever needed. The
mouse-move/up listeners live on `ChannelRack`'s root view (which fills
the available space), not on the small slider itself, so a fast mouse
movement that outruns the slider's own tiny hit area doesn't drop the
drag mid-gesture — same reasoning as the piano-roll prototype's
whole-window input div.

## What's genuinely new vs. what's scaffolding removed

New: `src/channel_rack.rs` (the view), the `AudioEngine` message-passing
rework, a native file picker via `rfd` for assigning samples.

Removed: `main.rs`'s `verify_persistence`/`verify_audio_engine` stderr
scaffolds from Stages 2 and 3 — this stage's UI is what they were
explicitly waiting for. Also deleted the stale test project
(`~/.local/share/allodium/projects/default/default.alp`) those scaffolds
had created, since it pointed at a nonexistent `samples/test.wav` and
served no purpose once real channels get created through the file
picker.

Autosave: every mutating action (step toggle, slider drag-end, mute/solo,
add/remove channel) calls `project::save_project("default", ...)`
immediately. This is not the same feature as the debounced-autosave +
undo-history system `02-feature-scope.md` lists as stretch scope — that's
a real architectural piece (history watcher, batched commits); saving
promptly after each change is just reusing Stage 2's existing
`save_project` as-is, with no new design needed.

## Verification — and its real limits

`cargo check` is clean. Ran the actual app twice: once with the stale
Stage 2/3 test channel present (confirmed the missing-sample path logs
and continues rather than panicking), once after deleting it (a genuinely
empty project, zero channels). Both held a steady ~60fps per `perf.rs`,
no panics either time.

**Not verified, and said plainly rather than glossed over**: anything
that needs a mouse, a keyboard, or ears. This environment has no way to
drive a native GPUI window's input or play audio through to a listener.
That means the file picker, step-toggle clicking, slider dragging,
mute/solo buttons, the step highlight actually tracking playback
position, and whether any of this is audible and correct are all
*unverified* beyond "the code path exists and the view renders." Stage 3
already owed one listening pass; this stage adds a full interactive pass
on top of it. Both are real, outstanding verification debt — worth
running through deliberately before trusting this stage's behavior, not
just its compile status.
