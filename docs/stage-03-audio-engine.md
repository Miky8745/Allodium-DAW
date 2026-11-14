# Stage 3: Audio engine, what was built

Status: done. `src/audio/` has device I/O, decoding, scheduling, and
mixing; verified via unit tests plus a live scheduler run, not yet by ear.

## Design choice: one clock, inside the callback

The original (`Allodium-tauri/src/lib/audioEngine.ts`) uses the Web Audio
API look-ahead scheduler pattern out of necessity: JS timers aren't
real-time, so it ticks every 25ms, schedules everything in a 100ms
look-ahead window against `AudioContext.currentTime`, and relies on that
separation to avoid missed/late triggers. The stage brief explicitly
invited reconsidering that two-clock split rather than porting it out of
habit, since native Rust doesn't have the same constraint: a `cpal`
output callback already *is* the real-time clock. `AudioEngine::play()`'s
callback computes `samples_played` (its own running counter, advanced
every output frame) directly into a step index each buffer, and triggers
anything starting at that exact step the moment it's crossed — no
separate timer thread, no look-ahead window, no risk of the two clocks
drifting apart, because there's only one.

## Design choice: an immutable snapshot per playback session

`PlaybackSnapshot` (channels flattened out of `ProjectFile`'s
pattern-scoped content, each carrying its own decoded `SampleBuffer`) is
built once, right before `play()`, and never mutated for that session's
lifetime. The original's engine takes live getter closures specifically
so BPM/channel edits apply without restarting — deliberately not ported
yet, because there's no UI that can edit a channel while it's playing
(Channel Rack and Piano Roll, both later stages). Building the lock-free
hot-swap path (likely a small SPSC channel feeding a new snapshot into
the callback, checked with a non-blocking `try_recv` at the top of each
buffer fill) before anything needs it would be speculative complexity.
Revisit this the moment Channel Rack actually wants to toggle a step
while the pattern is playing.

## The ogg-in-wav wrapper: confirmed real, not hypothetical

`Allodium-tauri/src-tauri/src/lib.rs`'s `unwrap_ogg_in_wav` exists because
some sample libraries ship a `.wav` file that's actually raw Ogg Vorbis
data sitting inside a WAV RIFF container (`fmt ` chunk format tag
`0x674F`) — a workaround for the browser's `decodeAudioData`, which can't
parse that. The stage brief said not to assume symphonia needs the same
workaround without checking. Scanned the full sample library (3176 `.wav`
files) for the tag directly: real affected files exist, under
`local_samples/Vocals/` (`Laurie Webb *.wav`, `VOC *.wav`). Ported the
unwrap step into `sample.rs` rather than skip it — it's a no-op for every
normal PCM file, so there's no cost to keeping it.

## What's in `src/audio/`

- `sample.rs` — `decode_sample` (symphonia probe + decode to interleaved
  `f32` PCM, with the ogg-in-wav unwrap applied first) and `SampleBuffer`.
- `pitch.rs` — `pitch_to_playback_rate` (`2^((pitch-60)/12)`, exact
  original formula) and `step_duration_secs` (`60/BPM/4`, sixteenth-note
  step convention).
- `envelope.rs` — `Envelope::ring` (step one-shots: ring to the sample's
  natural end, de-click fade-in only) and `Envelope::shaped` (piano-roll
  notes: decay/sustain/release fit to the note's length, with stage
  lengths clamped so a very short note doesn't produce a negative
  sustain) — same fixed constants and reasoning as the original's
  `playSample()`.
- `mod.rs` — `ChannelVoiceData`/`PlaybackSnapshot`/`build_snapshot`
  (flattening the project model for one pattern), `Voice` (one sounding
  one-shot: position, pitch-adjusted + sample-rate-corrected playback
  rate, equal-power pan, envelope gain), and `AudioEngine` itself (the
  `cpal` stream, the scheduler, `current_step()` for a future playhead).

Dual scheduling path, same as the original: a channel with any notes
plays only from that note data; a channel with empty notes falls back to
its boolean `steps` array at unshifted pitch. Mixing is linear-interpolated
at each voice's own fractional position, summed to a stereo pair, and
soft-clipped with `tanh` rather than hard-clamped so a loud mix compresses
instead of crackling (the original uses a real `DynamicsCompressor`; this
is a cheap stand-in, revisit if it's audibly insufficient once there's
real content to test against).

## Cross-thread sharing

`PlaybackState` (`playing`, `bpm_centibeats`, `samples_played`,
`sample_rate`, `pattern_length_steps`) is five atomics behind an `Arc`,
read/written by both the UI thread and the real-time audio callback with
no lock either side could block the other on — the stage brief was
explicit that the audio thread must never risk blocking on a `Mutex` the
UI thread might be holding.

## What's deferred (per the stage's own list)

Song-mode placement-based scheduling, per-channel independent looping,
slide notes, multisample folders, metronome, solo-channel audition
filtering. None of these are forgotten, just intentionally out of scope
for this stage.

## Verification

9 unit tests (pitch math, envelope shaping and cutoff timing, plus the
Stage 2 persistence tests) all pass. Ran the actual app with a synthesized
440Hz test tone (no hardcoded path to any other repo on this machine — see
`main.rs`'s `verify_audio_engine`, which accepts an optional
`ALLODIUM_TEST_SAMPLE` env var for a one-off manual check against a real
file) playing a 4-on-the-floor 16-step pattern at 120 BPM: `current_step()`
logged every 500ms advanced by roughly 4 steps each time (expected: 8
steps/sec at 120 BPM) and wrapped cleanly past the 16-step boundary, with
no `cpal` stream errors. Measured, rather than assumed, the stage's
explicit "restarting playback should be cheap" concern: 20 stop/play
cycles averaged ~24-27ms each — nowhere near the original's 450ms
"restart churn" bug (which was specifically in the song-mode-end
auto-restart path; no song mode exists yet here, so that exact bug has no
equivalent to even reproduce).

**Known gap**: no audible verification. This environment can't listen.
Correctness so far rests on the position/timing math, the unit tests, and
the absence of stream errors, not on anyone actually hearing clean
playback. Treat that as still owed once Channel Rack gives this real
sample content worth listening to.
