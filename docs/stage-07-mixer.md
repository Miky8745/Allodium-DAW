# Stage 7: Mixer, what was built

Status: done, at the "compiles, runs, holds 60fps" level of
verification — same caveat as every prior UI stage: nothing requiring a
mouse, keyboard, or ears has been confirmed yet.

## The real piece: wiring routing into the mixdown, not just a UI

`stages/07-mixer.md`'s "done when" is explicit that routing has to
*audibly* change what you hear, independent of a channel's own volume/
pan/mute — a UI-only `mixer_track` field (which already existed on
`Channel` since Stage 2, unused until now) wouldn't satisfy that. This
engine has no modular audio graph (unlike the original's Web Audio
`TrackChain` nodes) — everything mixes down inside one `cpal` callback —
so routing had to become a second mixdown stage inside that same
callback rather than a graph of nodes:

1. Every `Voice` now carries a `bus: u32` (`0` = Master, the default —
   "every channel routes to Master until explicitly assigned
   elsewhere," per the stage brief), resolved from
   `ChannelVoiceData::bus` (itself just `Channel::mixer_track`, finally
   read for the first time).
2. Per output sample-frame, voices accumulate into a small `Vec<f32>`
   pair (`bus_l`/`bus_r`) indexed by bus — one slot per `MixerTrack`
   plus a reserved last slot for Master — instead of a single `mix_l`/
   `mix_r` pair. No per-sample allocation: the vectors are only resized
   when the bus list itself changes (on an `UpdateMixer` message), not
   every frame.
3. A second pass folds each non-Master bus's own volume/pan/mute into
   Master's slot. A channel still routed straight to Master (`bus ==
   0`, untouched by this stage for most channels) already landed
   directly in Master's slot in step 2 and skips this pass entirely —
   which is exactly the "independent of the channel's own controls"
   requirement: only a channel explicitly routed to a track picks up
   that track's gain/pan/mute on top of its own.
4. Master's own volume (`ProjectFile::master_volume`, new field) is
   applied last, then the existing `tanh` soft-clip, unchanged.
5. `trigger_channel_step` (the shared pattern/song-mode trigger logic
   from Stage 6) didn't need to change — it already just builds
   `Voice`s, which now carry the extra `bus` field transparently.

**Design choice**: a bus's pan is a simple stereo **balance** control
(scale L/R independently), not the channel-level equal-power pan `Voice`
already uses for positioning a mono sample in the field. A bus's input
is already a stereo sub-mix by the time it reaches this stage, so
re-deriving an angle and `cos`/`sin`-ing it a second time doesn't apply
to a signal that's already got two channels — a balance control is both
simpler and the conventional treatment for a bus/group fader.

**Design choice, scope cut per the brief's own "single-target routing"
requirement**: `MixerTrack::routes_to` (added back in Stage 2, for a
track to feed another track) is **not** wired up this stage — every
track still resolves straight to Master in the mixdown's second pass,
full stop. The field stays in the data model and round-trips through
`.alp` files, but nothing reads it yet. `02-feature-scope.md` only asks
for channel → track → Master, not track → track chains, so this isn't a
regression against scope, just an honest note that the field is still
inert.

## Mixer multi-solo is genuinely different from Channel Rack's solo

The Channel Rack's own solo (Stage 4) is deliberately single-solo
(soloing one channel clears every other's). The Mixer's bus solo is
**multi**-solo — several buses can be soloed at once — per
`docs/mixer.md`'s documented "standard multi-solo mixer semantics."
`toggle_mixer_solo` just flips the one bus's own flag with no
side-effect on the others; `build_mixer_buses`' `any_soloed`/effective-
mute formula is the same "explicit mute always wins" shape as
`build_snapshot`'s channel version, just without the single-solo
exclusivity rule layered on top.

## Pattern creation happened in Stage 6; mixer-track creation happens here

Same situation as Stage 6 needing to add pattern-creation UI that didn't
exist yet: nothing before this stage needed more than Master to route
to, so `ChannelRack` grew `add_mixer_track`/`remove_mixer_track` (the
latter falling an orphaned channel's routing back to Master, matching
the original's "nothing goes silently unrouted" rule) alongside the
volume/pan/mute/solo setters the Mixer view calls into.

## Channel routing UI: a cycling pill, not a dropdown

`stages/07-mixer.md`'s scope is single-target routing with a small,
fixed set of destinations (Master + however many tracks exist) — a full
dropdown/menu is more machinery than that needs. Each Channel Rack row
got a small pill (`Mstr` / `Trk N`) that just cycles through every
available target on click (`ChannelRack::cycle_channel_route`), reusing
the same "no menu needed for a handful of options" reasoning Stage 5
already applied to channel selection.

## The Mixer view itself

`src/mixer.rs` is new design (no behavior-spec UI exists to port — the
original's cable-patchbay and click-to-route gestures are explicitly
*not* being carried over, per the stage's "multi-target routing... not
needed" cut). Vertical strips (Master + one per `MixerTrack`, `+ Track`
at the end), each with a vertical volume fader and (non-Master) a pan
dot and mute/solo pair. The fader/pan-dot drag mechanics are the same
relative-delta-from-mouse-down pattern as the Channel Rack's horizontal
sliders (`channel_rack.rs`'s `slider`) — same underlying GPUI
limitation (`div()`s don't expose their own bounds to event handlers),
just adapted to a vertical drag axis for the fader.

## What's explicitly not here

Insert-effect rack (not even empty placeholder UI), multi-target
routing, strip renaming, the toolbar oscilloscope/spectrum monitor — all
per the stage brief's own "explicitly not needed here" list.

## Verification — and its real limits

`cargo check` is clean. `cargo test` passes all 15 (12 prior + 3 new
ones covering `build_mixer_buses`' mute/solo folding and `bus_index`'s
fallback-to-Master behavior for an unknown/removed bus id — the kind of
logic a typo could silently break without a test catching it). Ran the
actual app: it opens with all four views (Channel Rack / Piano Roll /
Playlist / Mixer) stacked, holds a steady ~60fps per `perf.rs`
(16.6-17.0ms avg across several 120-frame windows), no panics, no `cpal`
stream errors.

**Not verified, same limits as every prior stage and still
accumulating**: this environment cannot drive a native window's mouse/
keyboard or play audio to a listener. Dragging a fader/pan dot, mute/
solo clicks, the channel-routing pill, adding/removing a mixer track,
and — the thing that actually matters for this stage's "done when" —
whether routing a channel to a track and adjusting that track's controls
audibly changes playback independent of the channel's own controls, are
all unverified beyond "the code path exists, compiles, and the mixdown
math has unit coverage." A real interactive + listening pass is still
owed, stacking on Stage 3/5/6's own outstanding verification debt. Given
how much audio-engine surface has now accumulated without ever being
heard, this is the strongest case yet for actually running that listening
pass before Stage 8.
