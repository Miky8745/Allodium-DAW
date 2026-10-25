# Bloom (instrument)

> Split out of `CLAUDE.md` to keep it under the size limit.

Bloom is Allodium's preset-based sample instrument and its first **plugin**
(`src/lib/plugins/registry.ts`). Assign it to a channel by right-clicking
the channel's sample field and picking **Bloom**. It can then play an **SFZ preset** (or keep its own sample/folder) and
shape it with an amp envelope, a filter with its own envelope, a tune knob
and up to 8 **macro knobs** defined by the preset.

## Data model

- `ChannelData.instrument: InstrumentSettings` (`src/lib/instrument/types.ts`)
  is saved and undo-tracked with the channel (`project.rs`'s `InstrumentXml`,
  container-level `#[serde(default)]`, no `Option`s; `presetPath: ''` = no
  preset). `macros` is a plain `number[]` in the store but a `{ value: [...] }`
  wrapper on the wire (quick-xml per-item tags). `exportInstrument()` and
  `normalizeInstrument()` convert at the boundaries. `normalizeInstrument()`
  also fills older saves and history snapshots.
- `ChannelData.plugin` (saved; `project.rs` `plugin: String`, `''` = none)
  says which plugin is assigned. Bloom plays only when `plugin === 'bloom'`
  and `instrument.enabled` (`bloomActive()`); the window's power button
  toggles `enabled`. `assignPlugin()`/`removePlugin()` call the plugin's
  `assign`/`remove` hooks (Bloom's remove resets `instrument` to defaults).
  On import, a channel with `instrument.enabled` but no `plugin` (saved
  before plugins existed) gets `'bloom'`.
- Otherwise the channel behaves **exactly** as before instruments
  existed: the engine's fixed envelope applies, and step hits ring out.
  `defaultInstrument()`'s envelope equals those fixed constants, so turning
  the instrument on for a plain sample changes nothing until a knob moves.
- **Envelope and filter are channel-wide, not per region.** Loading a preset
  (`applyPreset()`) copies its first region's `ampeg_*`, `fil_type`, `cutoff`,
  `resonance` and `fileg_*` into the channel (`presetDefaults()`), plus each
  macro's `set_ccN` default. From then on the knobs *are* the sound. Region
  volume, pan, tune and key mapping still apply per region.
- A preset (while Bloom is active) takes priority over the channel's `samplePath`/`sampleFolder`
  (`usesPreset()`/`channelHasSound()` in `instrument/channelSound.ts`).
  Dropping an audio file or folder onto a channel clears `presetPath`
  (`channelDrop.ts`); dropping an `.sfz` assigns Bloom and loads it as a preset.

## Presets

- `data/instruments/<Category>/<Name>.sfz`. Rust's `list_presets` (in
  `instruments.rs`) lists them recursively, and `read_text_file` reads one.
  The preset browser groups presets by their first folder. Presets elsewhere
  under `data/` still work when dropped from the file explorer.
- Sample paths resolve relative to the `.sfz` (plus `default_path`) and must
  stay inside `data/` (`resolvePath()`; Rust rejects `..`). The starter
  presets point at `../../samples/...` (the bundled GPL LMMS samples).
- **Macros are plain SFZ.** `<control> label_ccN=Name` makes a macro, and
  `set_ccN` (0–127) sets its default. Labeled CCs fill the 8 slots in CC
  order. A region's `<param>_onccN=<amount>` (also `<param>_ccN`) adds
  `amount × macro` to that parameter. The supported params are `cutoff`
  (cents), `resonance` (dB), `ampeg_attack|decay|release` (s),
  `ampeg_sustain` (%), `fileg_depth` (cents), `fileg_attack|decay` (s),
  `volume`/`gain` (dB), `pan`, and `pitch`/`tune` (cents). Modulations are
  read from the first region (`macroMods()`, cached per parsed preset).
  The same files work in other SFZ players' CC controls.
- **Supported opcodes** (`sfz.ts`): the headers `<control> <global> <master>
  <group> <region>` with inheritance; `#define $X`; `sample`, `default_path`,
  `key`/`lokey`/`hikey`/`pitch_keycenter` (note names use c4 = 60, which is
  Allodium's C5), `lovel`/`hivel`, `tune`, `transpose`, `volume`, `pan`,
  `offset`, `loop_mode` (`one_shot`, `loop_continuous`/`loop_sustain`),
  `loop_start`/`loop_end`, `seq_length`/`seq_position` (one counter per
  `<group>`), `lorand`/`hirand`, and `trigger` (only `attack` regions play).
  Not supported: `#include`, release triggers, embedded WAV `smpl` loops (a
  looping region without explicit points loops the whole file), crossfades,
  `amp_velcurve`, LFOs and effects opcodes. Unknown opcodes are ignored.
- All regions matching key and velocity play together (layers). Round robin
  and random selection filter that set (`regionSelect.ts`).

## Engine path

- `audioEngine.triggerPitched()` branches to `triggerInstrument()` when
  `ch.instrument.enabled`. `resolveInstrumentHits()` returns the hits (preset
  regions, or the channel's own sample/folder pick) plus the resolved
  `VoiceShape`. `buildInstrumentVoice()` (`voiceGraph.ts`) builds src → gain →
  [filter] → panner → mixer track. The amp/filter envelopes are in
  `envelope.ts`: a note shorter than its attack/decay releases from wherever
  the envelope got to.
- `triggerPitched` now takes a **velocity (0..1)**, not a pre-multiplied
  volume, because region selection needs the velocity.
- Step hits on an instrument channel play as **one-step notes**. SFZ
  `one_shot` regions ring out instead, ignoring note length and key-up.
- Piano Roll note props act relative to the instrument: mod X scales cutoff,
  mod Y adds resonance, and rel scales release (all neutral at defaults).
  Slides register every non-one-shot hit.
- Live voices (MIDI/QWERTY) hold `parts[]` (one per layered region) and
  release with the instrument's release (`releaseParam()`, using
  `cancelAndHoldAtTime` when available).
- Loop points and offsets are in source frames, but `decodeAudioData`
  resamples. `loadSample()` therefore records each file's original rate from
  its header first (`sourceRate.ts`: WAV/FLAC/Ogg), in
  `audioEngine.sourceRates`.
- `preloadChannel()` loads a preset and every sample it references.

## UI

- **Sample field menu**: right-clicking a channel's sample field opens
  `SampleFieldMenu.svelte` (the plugin list, with the assigned one checked,
  plus "Remove plugin"). It is separate from the row's menu. Picking a
  plugin assigns and opens it. **Double-clicking** the sample field opens the
  assigned plugin (`PluginDef.open`), or the Piano Roll when there is none.
  An assigned plugin's name is the field's default label; a rename wins.
- `InstrumentWindow.svelte` (floating window id `instrument`) edits
  `channelStore.selectedChannel`, like the Piano Roll (`instrumentWindow.open(id)`
  selects the channel). It has no Toolbar button.
- Left: `PresetBrowser.svelte` (search, rescan, which clears the parse
  cache). Clicking a preset loads it and auditions C5 for 0.9 s
  (`previewNote()`'s new `durationSec` arg). Right: `InstrumentPanel.svelte`
  holds the on/off power button, source name, Tune, Audition (▶) and Init
  (back to `defaultInstrument()`), followed by `MacroRow`, `AmpSection` (with
  `EnvelopeGraph`) and `FilterSection`.
- `ParamKnob.svelte` wraps `Dial` with a function binding over normalized
  0..1 travel, and `paramScales.ts` maps it to the real value (exponential
  for times and cutoff). Double-click or right-click resets to the default.
- The window header and source line show what actually plays
  (`formatSourceLabel()`: the preset or sample name).

## Ideas not done yet

Per-region envelopes, an LFO, velocity → filter, effects (reverb/delay/chorus,
possibly via the Mixer insert slots), hardware MIDI CC → macros, a user
preset save ("Save as .sfz"), embedded WAV loop points, and a waveform/key-map
view.
