# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**Keep this file up to date.** Whenever you make a change worth noting — new commands, architecture decisions, IPC additions, navigation quirks, data conventions — add it here before finishing the task.

**Size budget:** this file must stay well under ~150k chars (it's ~39k now). Long subsystem write-ups live in `docs/*.md` (`frontend-structure`, `playlist`, `midi`, `audio-playback`, `piano-roll`, `mixer`) — put new detail there and keep only a short summary + key gotchas here. Read the relevant `docs/` file before working on that subsystem.

## Commands

```bash
# Start Tauri dev app (launches Vite dev server + Rust backend together)
npm run tauri dev

# Frontend only (no Rust, browser at http://localhost:1420)
npm run dev

# Type-check frontend (run after changes to .svelte or .ts files)
npm run check

# Build for production (runs `npm run build` then compiles Rust)
npm run tauri build

# Build frontend only
npm run build

# Clear stale caches if the UI shows stale content/404s after a dependency
# change — clears WebKitGTK's own webview cache
# (~/.local/share/com.miky8745.allodium/{WebKitCache,CacheStorage}, not
# touched by a plain Vite/SvelteKit cache clear) plus node_modules/.vite
# and .svelte-kit
./scripts/clear-cache.sh
```

**Background launch** — `ALLODIUM_BACKGROUND=1 npm run tauri dev` starts the window minimized (Rust `setup`) and keeps it minimized after `openProject`'s `maximize()` (frontend asks the `background_launch` command; needs `core:window:allow-minimize`). Use this when launching the app for verification so it doesn't pop over the user's screen.

**Profiling playback** — `scripts/profile.sh <panels> <seconds> <log> [project]` runs the app headless on Xvfb (`GDK_BACKEND=x11`, no window on the user's screen), autoplays a project muted (Song mode, then pattern mode as a baseline), quits, and prints a per-phase fps/stall summary; `<log>` keeps the full `[perf]` output. `panels` = `none` | `all` | comma list of `channelrack,pianoroll,mixer,overview,noplaylist` — bisect by window to find what's expensive. Driven by env vars read in `src-tauri/src/debug.rs` (`ALLODIUM_PROFILE`, `ALLODIUM_AUTOPLAY`, `ALLODIUM_AUTOPLAY_SECONDS`, `ALLODIUM_AUTOPLAY_PANELS`); the frontend side is `src/lib/debug/` — `profiler.ts` (no-op unless started: rAF frame pacing, main-thread stall detection via a late 10ms interval with attribution to the instrumented sections that ran, MutationObserver counts + most-mutated elements, per-second `begin()/end()` section stats, `count()` counters, gauges) and `autoplay.ts`. Instrumented: `audio.tick`, `clock.raf`, `history.watch`/`history.stringify`, derived recomputes, voice/stale-drop counters, scheduler lead. **Don't profile a minimized window** (`ALLODIUM_BACKGROUND`): WebKitGTK throttles a hidden page's timers to ~1s and stops rAF — the numbers are garbage, and for the same reason playback genuinely falls apart if the user minimizes the app while playing (Tauri's `backgroundThrottling` is unsupported on Linux; not fixed).

Rust backend is built/run via `npm run tauri dev` — there is no separate `cargo run` entry point for the app. To check Rust code in isolation: `cd src-tauri && cargo check`.

## Code style

**Prefer many small files over few large ones.** If a component, helper, or type block is getting long, extract it into its own file without hesitation. Route pages should be thin coordinators — logic and UI belong in focused components under `src/lib/`.

## Architecture

This is a **Tauri v2 desktop app** with two distinct layers:

**Frontend** (`src/`): SvelteKit in **SPA mode** (SSR disabled, `adapter-static` with `fallback: "index.html"`). Svelte 5 runes syntax (`$state`, `$derived`, `$effect`, etc.) is used throughout. All routes live under `src/routes/`. There is no server-side code — SvelteKit is used purely as a client-side framework bundled by Vite.

**Backend** (`src-tauri/`): Rust crate (`allodium_lib`). The Tauri app is initialized in `src-tauri/src/lib.rs`; `main.rs` just calls `allodium_lib::run()`. Tauri commands are defined with `#[tauri::command]` — most in `lib.rs`, but project-file persistence commands live in `src-tauri/src/project.rs`, global preferences in `src-tauri/src/preferences.rs`, MIDI I/O in `src-tauri/src/midi.rs`, and system-audio capture in `src-tauri/src/capture.rs` (each a separate module) — and all are registered via `invoke_handler!` in `lib.rs`. `lib.rs`'s `setup` also `.manage()`s the MIDI state and starts its two background threads (see "MIDI"). Rust unit tests: `cd src-tauri && cargo test --lib` (preferences XML round-trip, MIDI port-key parsing, `.alp` note backward-compat/round-trip); `cargo test --lib -- --ignored capture` smoke-tests system-audio capture against the running PulseAudio/PipeWire server. The frontend calls them with `invoke("command_name", { args })` from `@tauri-apps/api/core`.

**IPC boundary**: Frontend → backend is always Tauri's `invoke` mechanism; backend → frontend only uses Tauri events for MIDI (`midi-message`, `midi-ports-changed`, see "MIDI"). When adding new backend functionality, define the command in `lib.rs` and register it in `invoke_handler!`.

Vite is fixed to port **1420** (required by Tauri's `devUrl` config) — the port must be available when running dev.

## Frontend structure

**Full per-file map: [`docs/frontend-structure.md`](docs/frontend-structure.md)** — read it before adding/locating a store, helper or component. Layout in brief:

- `src/routes/+page.svelte` — the single page (home + project views); `+layout.svelte` is a pass-through. `app.html` holds the CSS reset + design tokens (`app.css` is a reference copy, not imported).
- `src/lib/*.svelte.ts` — class-based singleton stores (`channelStore`, `patternStore`, `placementStore`, `playbackStore`, `historyStore`, `mixerStore`, `mixerAudio`, `windowFocusStore`, `hoverInfoStore`, `preferencesStore`, `noteDefaultsStore`, `noteColorStore`, `pianoRollZoom`, `songTimeline`, …). Project-state stores expose `exportState()`/`importState()`/`resetToDefault()`.
- `src/lib/*.ts` — pure helpers (`audioEngine`, `projectSerializer`, `channelLoop`, `multisample`, `placementLayout`, `deleteFx`, `dragCursor`, `portal`, `colorUtils`, `meterMath`, `noteProps`, `slideVoices`, `sampleName`, …); `pianoroll/pitch.ts` is the pixel/pitch geometry source of truth; `midi/` = frontend MIDI; `debug/` = profiler.
- `src/lib/components/` — shared controls (`Dial`, `Knob`, `ScrollField`, `Slider`, `ColorWheel`, `DragTooltip`, `KnobContextMenu`, `FloatingWindow`, `EditToolButtons`, `Toolbar`, `MenuBar`, `FileExplorer`, …) plus subfolders `pianoroll/`, `playlist/`, `mixer/`, `preferences/`, `midi/`.
- Floating overlays that must escape `overflow` clipping are portalled onto `<body>` via `portal.ts` (tooltips, context menus, menu dropdowns).


**Visual language ("dusk")** — a single soft dark theme: blue-slate neutrals (`--main-bg #1f222b`, panels `#20232d`, borders `#333747`; contrast deliberately compressed, no pure black/white) with one warm peach accent (`--accent #e6b07f`; text on an accent fill uses `--accent-text`, a dark warm brown, not white). Corners are generously rounded — small controls 4–5px, notes/steps ~7–8px, windows 9px+; use the same scale for new components instead of 2–3px. Semantic colors stay semantic: level meters (`meterMath.ts`) are still green→yellow→red, record/mute stay red — only softened. `--toolbar-green-bg` is a legacy name for the slate menu-section fill. `app.css` mirrors `app.html`'s tokens (single theme, no light mode).

**Cursors** — one convention app-wide: Piano Roll Draw tool = brush (`var(--cursor-brush)`), Delete tool = eraser (`var(--cursor-eraser)`) — both hand-drawn SVG data-URI tokens in `app.html`/`app.css`, hotspot at the tip; Marquee tool = `crosshair`; hovering an extendable edge = `col-resize` (Channel Rack name column, explorer handle, a note's right edge in Draw — `NoteGrid`'s `hoverEdge`), window resize handles keep their edge cursors; **while dragging** something movable = `move` for every-direction moves (notes, placements, pattern cards, file-explorer samples, floating windows) and `ew-resize` for horizontal-only moves (both overview windows); otherwise the regular cursor (`grab`/`grabbing` are no longer used). Drag cursors go through `src/lib/dragCursor.ts`'s `beginDragCursor(cursor)`/`endDragCursor()`, which set `--drag-cursor` + an `html.drag-cursor` class whose `!important` rule (in `app.html`) keeps that cursor everywhere until the drag ends, even off the element that started it. Pair every begin with an end on every exit path (pointerup *and* the `e.buttons === 0` fallbacks).

CSS custom properties (design tokens) live in `app.html`'s `<style>` tag so they are available synchronously before any JS runs. New components should use `var(--token)` rather than hardcoding colors.

**No native right-click menu** — an inline `<script>` at the top of `app.html`'s `<body>` `preventDefault()`s every `contextmenu` event in the capture phase, so the webview's Inspect Element/Reload menu never appears (home and project views alike, even inside text inputs). Components' own right-click behavior is unaffected; nothing in the app reads `e.defaultPrevented`. Devtools in dev builds: Ctrl+Shift+I.

The global reset in `app.html` (mirrored in `app.css`) includes `:focus, :focus-visible { outline: none; }` — there is no browser focus ring anywhere in the app (it otherwise shows on whatever was last clicked once a key like Space is pressed). Deliberate outline visuals (e.g. `.step--playing`) are unaffected. The reset also sets `user-select: none` on `html, body` so dragging around the UI never drag-selects text (matches a native desktop app feel); `input`/`textarea` are explicitly reset back to `user-select: text` so form fields stay usable.

## Playlist

**Details: [`docs/playlist.md`](docs/playlist.md)** (patterns panel, rename/recolor/delete, tools & block selection, placing/moving, previews, overview bar, playhead, start marker). Key points:

- Patterns panel docked left inside the Playlist window; 8 fixed tracks (local `$state`, not persisted). `patternStore` is shared with the Toolbar's pattern dropdown. Deleting patterns goes through `patternActions.ts`'s `deletePatterns()` (also removes placements) — right-click only; the Delete key deletes *selected placed blocks*.
- `Placement.kind: 'pattern' | 'clip'` with **sentinel fields** (`patternId: 0`, `samplePath: ''`, `lengthBeats: 0`) instead of `Option`s, to dodge quick-xml's `None → Some("")` quirk. A pattern placement's length is always read live from `patternStore.effectiveLengths`.
- Tools live in `playlist/playlistEditor.svelte.ts` (Draw/Delete/Marquee, same semantics as the Piano Roll); right button = delete in every tool. No overlapping blocks: placement goes through `placementLayout.ts`'s `nearestFreeStart()`/`groupFits()`. Grid = one *beat* per cell (`beatWidth`), shaded per bar.
- **Don't add a document-wide `pointermove` listener** for drop positions — `FileDropDetail` carries the drop's `x`/`y`.
- **Per-frame updates (playheads, step highlights) must not live inside an `overflow: auto` scroller and must use `transform`** — otherwise WebKitGTK re-renders the scroller and every floating window over it each frame (measured 25fps → 62fps). Playheads are overlays outside the scroller.
- Song start marker: pass `seekStep` as `startSong()`'s `startStep` arg — **never seek with a separate call right after `start()`/`startSong()`** (they're async and reset `scheduledStep`).
- Keyboard handlers (Space, Delete, Ctrl+A) are gated on `windowFocus.isFrontmost('playlist')` so they don't collide with the Piano Roll's.

## Preferences

Blender-style **Edit → Preferences…** (also the home Sidebar's Settings button, and **Ctrl+,** in either view — `+page.svelte`'s `onGlobalKeydown`). `PreferencesModal.svelte` is mounted once in `+page.svelte` outside both `view` branches. No OK/Cancel: every change applies live and `preferencesStore` debounce-saves it.

- **Global, not per-project, not undo-tracked** — `data/preferences.xml`, loaded in `+page.svelte`'s `onMount` (before anything else) and followed by `preferences.startWatching()`, `startApplyingPreferences()`, `midiSystem.start()`. Rust side (`preferences.rs`) is `#[serde(default)]` on every struct so older files load; no `Option` fields (sentinels instead: `lastProject: ''`, `sampleRate: 0` = auto, `channel: 0` = omni, `fixedVelocity: 0` = off) to sidestep quick-xml's `None → Some("")` quirk.
- **General tab** — GUI scale (preset `PrefSegmented`, applied via `getCurrentWebview().setZoom()`; needs `core:webview:allow-set-webview-zoom` in `capabilities/default.json`); audio latencyHint + sample rate (construction-only AudioContext options → `audioEngine.rebuildContext()`, which stops playback, closes the old context, clears track chains and rebuilds; decoded AudioBuffers survive. `mixerAudio` listens via `audioEngine.onContextRebuilt()` and restarts itself with a fresh `lastRoutes`, re-creating every chain/edge) + a live base/output latency readout; scheduler look-ahead/tick (`audioEngine.setSchedulerTiming`, formerly the `LOOKAHEAD`/`INTERVAL` constants); autosave on/off (`historyStore.autosaveEnabled` is now a getter/setter over `preferences.general.autosaveEnabled`, so File › Autosave and this switch are the same thing), idle delay and undo history size (were `IDLE_MS`/`MAX_ENTRIES`); reopen last project on launch (`lastProject` is set in `openProject`); new-project default BPM/beats-per-bar (applied in `openProject` only when no `.alp` exists yet); Piano Roll preview volume/length (`audioEngine.setPreview`, formerly `PREVIEW_VELOCITY`/`PREVIEW_DURATION`); QWERTY keyboard velocity; audio monitor source (`monitorSource: 'app' | 'system'`, consumed by `BlackBox.svelte` — see "Toolbar audio monitor & peak meter").
- **Output device selection is not implemented** — WebKitGTK has no `AudioContext.setSinkId`, so Web Audio always plays to the system default. The General tab shows this as a read-only row.

## MIDI

**Details: [`docs/midi.md`](docs/midi.md).** Runs in Rust (`midir`, ALSA) — WebKitGTK has no Web MIDI. Port *keys* strip the volatile `client:port` suffix and are the identity everywhere (prefs, events, `ChannelData.midiOutPort`). Backend threads: hot-plug poller (emits `midi-ports-changed`) + output scheduler. `midiSystem.svelte.ts` reconnects enabled ports by name. Input processing is pure (`midiProcessing.ts`); all real-time sources (hardware + QWERTY keyboard) go through `liveNotes.svelte.ts`, which plays the selected channel and drives the Piano Roll key glow. Output sends **piano-roll notes only** (never step hits) via `audioEngine`'s `NoteSink` → `midiOutput.ts` → `midi_schedule`. Pitch bend is live-only.

## Navigation

**There is no cross-page navigation.** Both the home view and the project workspace live in `src/routes/+page.svelte` and are toggled via a `view` state variable (`'home' | 'project'`). This avoids Vite dev-mode CSS race conditions caused by full page reloads.

**Do not add `window.location.href` navigation or `goto()` calls** to switch between home and project — mutate `view` instead and call the Tauri window APIs (maximize/restore/setTitle) directly in the transition functions `openProject` / `exitProject`.

## Data storage

All app data lives under `data/` in the repo root. The Rust backend resolves this via `app_root()` in `lib.rs`, which calls `std::env::current_dir()` and steps up one level if the cwd is `src-tauri/` (which it is during `tauri dev`). `projects_root()` calls `app_root()` and appends `data/projects/`.

- **Projects**: `data/projects/<project-name>/` — one folder per project, containing:
  - `<project-name>.alp` — the saved project (XML). Written by `save_project`, read by `load_project`. Absent until the first save. Builds before the rename to Allodium wrote `<project-name>.vlp` (identical format); `load_project` falls back to it when no `.alp` exists, and the next save writes the `.alp`.
  - `history` — the undo/redo stacks (plain JSON, not XML — Rust treats its contents as an opaque string). Written/read by `save_history`/`load_history`.
- **Preferences**: `data/preferences.xml` — global, not per-project (see "Preferences"). Written by `save_preferences`, read by `load_preferences`.
- **Samples**: `data/samples/` — 240 audio files from LMMS's sample library (basses, bassloops, beats, drums, effects, instruments, latin, misc, shapes, stringsnpads, waveforms), **GPL-2.0-or-later** (`data/samples/LICENSE`, credits + upstream-rename table in `data/samples/CREDITS.md`); the app code itself is MIT. A few files were renamed to drop hardware brand names. LMMS's `drumsynth/` `.ds` presets were removed (the app can't play them).
- **Licensing rule for bundled content** — only commit sounds/assets that may be redistributed (CC0 preferred, CC-BY/GPL with a license + credit file next to them). Never commit third-party DAW factory content or anything with no-redistribution/NC terms; `data/samples/local_samples/` is gitignored for a local-only user library. Don't name third-party DAW products in code, comments or docs.
- **Icons** — `src-tauri/icons/` (+ `gen/android` launcher icons) and `static/favicon.png` are generated from the leaf logo (same SVG as `MenuBar`/`Sidebar`): render it to a 1024px PNG with `rsvg-convert`, then `npx tauri icon <png>`.

## Tauri commands

Most commands are defined in `src-tauri/src/lib.rs`; project-file persistence commands live in `src-tauri/src/project.rs`. All are registered in one `invoke_handler!` call in `lib.rs`.

| Command | Args | Returns | Description |
|---|---|---|---|
| `debug_config` | — | `DebugConfig` | Profiling/autoplay env vars (Commands › Profiling) |
| `perf_log` | `lines: Vec<String>` | `()` | Prints `[perf]` lines to stderr (the profiler's sink) |
| `debug_exit` | — | `()` | Quits the app (end of an autoplay run) |
| `background_launch` | — | `bool` | Whether `ALLODIUM_BACKGROUND` is set (see Commands › Background launch) |
| `list_projects` | — | `Vec<String>` | Sorted list of project folder names |
| `create_project` | `name: String` | `()` | Creates `data/projects/<name>/` |
| `list_project_files` | `name: String` | `Vec<FileNode>` | Recursive file tree for a project (dirs first, max depth 10) |
| `list_data_files` | — | `Vec<FileNode>` | Recursive file tree for the entire `data/` directory |
| `get_data_root` | — | `String` | Absolute path to the `data/` directory |
| `read_audio_bytes` | `relative_path: String` | `tauri::ipc::Response` (raw binary → `ArrayBuffer` in JS) | Reads `data/<relative_path>` and returns raw bytes (path traversal blocked). **Must stay a binary `Response`, never `Vec<u8>`** — a `Vec<u8>` is JSON-encoded as a number array (~4x size, parsed on the main thread), which froze the UI on every sample load. Some sample libraries ship files as a RIFF/WAVE shell with format tag `0x674F` (Ogg Vorbis mode 1) whose "data" chunk is actually a raw Ogg Vorbis stream browsers don't recognize wrapped as-is; this command unwraps that shell down to the plain Ogg payload before returning (`unwrap_ogg_in_wav` in `lib.rs`) — a no-op for ordinary PCM WAVs |
| `list_dir_files` | `relative_path: String` | `Vec<String>` | Non-recursive list of audio-file names (`wav`/`ogg`/`mp3`/`flac`) directly inside a data-relative folder, sorted. Used to build a multisample instrument when a folder is dropped onto a channel (see "Multisample instrument folders" below) |
| `save_project` | `name: String, project: ProjectFile` | `()` | Serializes `project` to XML and writes `data/projects/<name>/<name>.alp` |
| `load_project` | `name: String` | `Option<ProjectFile>` | Reads/parses `<name>.alp`; `None` if it doesn't exist yet |
| `save_history` | `name: String, contents: String` | `()` | Writes the opaque JSON `contents` string to `data/projects/<name>/history` |
| `load_history` | `name: String` | `Option<String>` | Reads `history` as a raw string; `None` if it doesn't exist yet |
| `load_preferences` | — | `Option<Preferences>` | Reads/parses `data/preferences.xml`; `None` if absent |
| `save_preferences` | `prefs: Preferences` | `()` | Writes `data/preferences.xml` (pretty-printed XML) |
| `midi_list_ports` | — | `MidiPorts` | `{ inputs, outputs, connectedInputs, connectedOutputs }` — port *keys* (see "MIDI") |
| `midi_set_inputs` | `names: Vec<String>` | `MidiPorts` | Diffs open input connections against `names` (connects/disconnects) |
| `midi_set_outputs` | `names: Vec<String>` | `MidiPorts` | Same for outputs; a closed output first gets note-offs for anything still sounding |
| `midi_send` | `port: String, bytes: Vec<u8>` | `()` | Sends one message immediately |
| `midi_schedule` | `events: Vec<{port, bytes, delayMs}>` | `()` | Queues messages on the scheduler thread, each `delayMs` from now |
| `midi_stop_all` | `cc123Ports: Vec<String>` | `()` | Clears the queue, note-offs every note that went out but hasn't been released, CC123 on the listed ports |
| `system_audio_start` | `sampleRate: u32, onData: Channel<InvokeResponseBody>` | `()` | (Re)starts capturing `@DEFAULT_MONITOR@` (`capture.rs`, libpulse-simple — needs the libpulse dev package to build); streams raw mono LE f32 chunks (480 frames) as `ArrayBuffer`s |
| `system_audio_stop` | — | `()` | Stops capture (joins the thread) |

`FileNode` is `{ name: String, is_dir: bool, children: Vec<FileNode> }`. `ProjectFile` and its nested structs are defined in `project.rs` — see "Undo/redo & saving" below.

## Audio / playback

**Details: [`docs/audio-playback.md`](docs/audio-playback.md)** — also covers the Toolbar audio monitor & peak meter, dropping samples into the Channel Rack, the Channel Rack row context menu, growing/trimming the step sequencer, and multisample instrument folders. Key points:

- `playbackStore.svelte.ts` is the single playback clock: owns all `audioEngine.start()`/`startSong()`/`stop()` orchestration and the rAF loop (`currentStep`/`currentStepFraction`), independent of which windows are open. `transportMode` (`'pattern' | 'song'`) and `metronomeOn` live here.
- `audioEngine.ts` (look-ahead scheduler) takes **getters, not values**, so live BPM/channel/length changes apply without a restart; it never imports stores itself. Song mode: `songTimeline.svelte.ts` → `getSongPlacements()`; plays once and stops at the end; pattern mode loops.
- **`patternStore.effectiveLengths` and `channelStore.activeLength` are memoized `$derived`s** — the scheduler reads them up to 80×/s; never make them recompute per read.
- Loop counters are kept bounded and re-wrapped one cycle at a time, so a growing `activeLength` never makes the position jump. `patternLength` = editable canvas; `activeLength` = content-driven loop bound. Each channel loops on its own content (`channelLoop.ts`'s `channelLoopLength()`, shared by engine and UI).
- A channel with any `notes` plays only from notes; otherwise from `steps`. Pitch 60 = C5 = rate 1. Channel volume range is `[0, 2]`.
- Channel Rack solo (`ChannelData.soloed`, Ctrl+click mute dot) is single-solo and persisted — separate from `playback.soloChannelId` (Piano Roll audition filter) and the Mixer's solo.
- quick-xml round-trips `Option::None` as `Some("")`: normalize at import with `||`, not `??`.
- Samples load via `read_audio_bytes` (binary) and are cached; channels are preloaded on project open / sample drop so the first hit isn't dropped by the 150ms staleness cutoff (previews pass `allowLate`).
- Per-pattern step/note content: channels are shared across patterns; `channelStore.switchPattern()` (called by `patternStore`'s `selectedPatternId` setter) stashes/restores content.

## Views (both in `src/routes/+page.svelte`)

### Home view (`view === 'home'`)
- Sidebar with Projects / Customize / Plugins / Learn navigation.
- Projects panel: search bar, scrollable project list (loads via `list_projects` on mount), empty state when no projects exist.
- **New Project** button opens an in-page modal. On confirm, `create_project` is invoked then `openProject()` is called to switch to the project view.
- Window size: 800×600 (set in `tauri.conf.json`).

### Project view (`view === 'project'`)

The workspace is divided into three fixed zones. **This hierarchy must be respected for all future additions.**

```
┌──────────────────────────────────────────────────────────────┐
│  TOOLBAR ROW 1 (~50px, CSS grid col1+col2)                   │
│  col1 [slate]  : logo File Edit Add Patterns View … Help  │
│  col2: [PAT|SNG] [▶][■][⏸][●] [BPM] [POS] [⌨→🎹][321][↺][🎵]│
│        [monitor rect] [peak meter]                           │
├──────────────────────────────────────────────────────────────┤
│  TOOLBAR ROW 2 (~26px)                                       │
│  col1 [info panel]: last-hovered control's name/value        │
│  col2: [📁][≡CR][🎹PR][▦PL][🎛MX][↩AS][🎤] [Snap▾][Pat▾]   │
│        [MIDI●][SHF][ALT][CTL]                                       │
├──────────┬───────────────────────────────────────────────────┤
│  LEFT    │  WORKSPACE                                        │
│  File    │  (everything the user edits/plays with)           │
│  explorer│                                                   │
└──────────┴───────────────────────────────────────────────────┘
```

- **Toolbar** (`Toolbar.svelte`) — a CSS grid (`grid-template-columns: max-content 1fr; grid-template-rows: 50px 26px`). Col 1 is the menu section (grey-green bg, `--toolbar-green-bg`); it auto-sizes, and the info panel in row 2 col 1 stretches to the same width automatically. All transport/toggle/panel buttons live in col 2. New toolbar buttons go in `Toolbar.svelte`.

- **Info panel (row 2 col 1)** — `.tb-info`, reads the `hoverInfo` singleton (`src/lib/hoverInfoStore.svelte.ts`): line 1 is `hoverInfo.name`, line 2 is `hoverInfo.value`, both blank (`null`) until anything's ever been hovered. Populated by the `hoverInfoAction` Svelte action (`src/lib/hoverInfo.ts`), attached via `use:hoverInfoAction={{ name, value }}`: `mouseenter` writes the panel and the action's `update()` keeps `value` live while still hovered (e.g. dragging a Dial without moving off it updates the panel in step with the drag, not just a one-time snapshot at mouseenter). **`mouseleave` deliberately does not clear the panel** — it keeps showing the last-hovered control's name/value until something else is hovered, rather than reverting to any idle placeholder. Wired into:
  - **`Dial.svelte`/`Knob.svelte`/`ScrollField.svelte`/`Slider.svelte`** — each gained a `name?: string` prop (default `''`, which opts out); the action reads `{ name, value: format(value) }` (or `displayWithLabel` for `ScrollField`, matching its own `DragTooltip` text) so the panel always reflects the control's *current* value, live during a drag. Every call site across `ChannelRow`/`ChannelRack`/`MixerChannelStrip`/`Playlist` passes a concrete name (e.g. "Pan dial", "Volume dial", "Mixer track", "Time signature", "Pattern length"); the Mixer's Pan dial/Volume fader use a `stripName` derived value (`data.name`, or `"Master"` for the Master strip, which has no `name` field of its own) so multi-strip hovers read e.g. "Kick pan dial" rather than a generic label.
  - **The Toolbar's own controls** — every button/toggle/dropdown/display in `Toolbar.svelte` (transport, PAT/SONG, BPM, POS, feature toggles, panel-visibility buttons, snap/pattern dropdowns, SHF/ALT/CTL indicators), plus `BlackBox.svelte`'s monitor button and `PeakMeter.svelte`'s meter, carry the same `use:hoverInfoAction`. Toggle-style controls report `"On"`/`"Off"` (or a more specific pair like `"Playing"`/`"Stopped"`) as their value; pure momentary actions (Stop) report a name only.
  - Not wired up elsewhere (Playlist blocks, Piano Roll notes, file tree rows, Mixer mute/solo/route buttons, etc.) — scoped deliberately to the shared value-controls plus the Toolbar's own controls, not literally every hoverable element in the app.
- **Left** — file explorer and any future list-type structures.
- **Workspace** — everything the user edits: playlist, piano roll, mixer, etc.

**Never place controlling buttons or displays in the left or workspace zones.** New toolbar controls belong in `Toolbar.svelte`; new left-panel structures belong inside/beside `FileExplorer.svelte`.

Entered via `openProject(name)`: sets window title, maximizes, loads file tree, loads the project's saved `.alp` (or resets all stores to blank if none exists yet), initializes and starts the undo/redo history watcher and the `playbackStore` clock watcher, sets `view = 'project'`. File → "Exit project" calls `exitProject()`: stops the history watcher and the clock watcher, unmaximizes, restores 800×600, centers, resets title, sets `view = 'home'`.

## Undo/redo & saving

- **One debounced watcher drives both undo/redo history and autosave**, rather than instrumenting every mutation call site (which are scattered across `NoteGrid.svelte`, `ChannelRow.svelte`, `ChannelRack.svelte`, `Playlist.svelte`, and the stores themselves). `historyStore.svelte.ts`'s `startWatching()` opens a standalone reactive root via `$effect.root()` (required since this is a plain module singleton, not a component — a bare `$effect` has no parent effect context to attach to) containing an `$effect` that re-runs on any project-state change and (re)arms an idle `setTimeout` (Preferences' autosave delay). When it fires with no further changes, it stringifies the project and, if it differs from the last-committed JSON, commits: pushes the pre-change snapshot onto the undo stack (`#past`, capped at the undo limit, oldest dropped first), clears the redo stack (`#future`), schedules a `history` file write, and — if `historyStore.autosaveEnabled` (on by default, toggled via File → "Autosave: On/Off") — also calls `saveProject()`.
- **`undo()`/`redo()`** (`historyStore.svelte.ts`) are standard past/future stack swaps built on `applyProjectState()`.
- **What counts as project state** (saved to `.alp` *and* snapshotted for undo) — `src/lib/projectSerializer.ts`'s `ProjectFileData`: `channelStore` (channel settings + every pattern's steps/notes, beatsPerBar, patternLength, stepCount — see "Growing the step sequencer"), `patternStore` (patterns + selectedPatternId), `placementStore` (playlist placements), and `playback.tempo`. Explicitly excluded: `isPlaying`/`currentStep`/`currentStepFraction`/`soloChannelId`/`transportMode`/`seekStep` (playback-only / ephemeral — the Playlist start marker and the PAT/SONG toggle reset each session) and any UI selection state (e.g. `channelStore.selectedChannelId`) — these are ephemeral and reset per session.
- **Why every store needs `exportState()`/`importState()`** — `channelStore`'s per-pattern step/note content lives in a private `Map` (not JSON/XML-serializable), and it's the only place that knows how to flatten it (merging the currently-active pattern's *live* content, which isn't in the map, with every other pattern's *stashed* content, which is). `patternStore.importState()` sets `#selectedPatternId` directly rather than through its public setter, which would otherwise call `channelStore.switchPattern` and corrupt the import (that swap logic is exactly what full-state import needs to bypass — it already restores every pattern's content wholesale). All four stores recompute their module-scope `nextId` counter after import (`max(existing ids) + 1`) so newly created objects post-undo/load never collide with restored ids.
- **XML on the Rust side** (`src-tauri/src/project.rs`, using `quick-xml`'s serde support) — every struct is `#[serde(rename_all = "camelCase")]` so field names match the plain JSON object the frontend passes to `invoke('save_project', { name, project })` (Tauri IPC args are always JSON, regardless of what Rust does with them afterward — this attribute is what makes e.g. `sample_path`/`samplePath` line up). quick-xml repeats a bare `Vec<T>` field's own tag name per element rather than giving each item its own tag, so the four top-level lists (`channels`, `content`, `patterns`, `placements`) are each wrapped in a small struct with a renamed inner field (e.g. `ChannelsXml { #[serde(rename = "channel")] items: Vec<ChannelSettings> }`) to get clean per-item XML tags. `projectSerializer.ts`'s `ProjectFileData` mirrors this wrapper shape exactly (e.g. `channels: { channel: [...] }`) so it deserializes cleanly on the Rust side.
  - **`.alp` is written pretty-printed** — `save_project` uses `quick_xml::se::Serializer::new(&mut body)` with `.indent(' ', 2)` (2-space indent) rather than the one-line output of `quick_xml::se::to_string`, purely so a saved project is readable/diffable by hand. `load_project` is unaffected — `quick_xml::de::from_str` is whitespace-tolerant either way, and this only touches serialization. Note the `Option::None`-round-trips-as-`Some("")` quirk documented under "Sample paths" above still applies regardless of indentation.
- **The `history` file is deliberately untyped on the Rust side** — `save_history`/`load_history` just read/write a plain JSON string; only `.alp` needs to be real XML, so there's no reason for Rust to understand the history file's internal shape (avoids a second parallel struct family).
- **Keeping commits cheap (was a visible UI stall ~1s after every edit)** — the stacks are private plain arrays of JSON *strings* (`#past`/`#future`), not `$state`: nothing renders them, and as deep `$state` proxies every commit re-walked and re-stringified up to `undoLimit` full snapshots. The watcher effect only calls `serializeProject()` (that read is what subscribes it to all project state) and re-arms the idle timer; the single `JSON.stringify` + compare + push happens in `#commitIfChanged()` when the timer fires. The `history` file is built by joining the stored strings (same `{"past":[…],"future":[…]}` shape as before) and written at most once per `PERSIST_DELAY_MS` (3s); `stopWatching()` flushes a pending write. Don't make the stacks reactive again or re-stringify them wholesale.

## Floating windows

Playlist/ChannelRack/PianoRoll all wrap their content in the shared `FloatingWindow.svelte` (drag/resize/maximize chrome). Stacking order (which window renders on top when they overlap) is decided by **focus, not DOM/mount order**: `windowFocusStore.svelte.ts` (singleton `windowFocus`, ephemeral/session-only) holds an `order: string[]` array (back-to-front); `FloatingWindow` requires an `id` prop and derives its own `z-index` from `windowFocus.zIndexOf(id)` inline (a component's own `<style>` block can't express a per-instance dynamic value, so this rides in `rectStyle`'s existing inline `style="..."` string alongside position/size). A single `onpointerdown` handler on the outer `.fw` wrapper calls `windowFocus.focus(id)` — since `pointerdown` bubbles from the header, body, and resize handles alike (the header's own `onmousedown={onDragStart}` and the resize handles' `mousedown` + `stopPropagation()` are a different event type and don't block it), clicking anywhere in a window brings it to front, not just its header.

Toolbar's Channel Rack/Piano Roll/Playlist/Mixer toggle buttons implement 3-state taskbar-style semantics instead of a bare open/closed flip: closed → open **and** focus; open-but-not-frontmost → bring to front (**stays open**); open-and-already-frontmost → close. `+page.svelte`'s `onOpenPianoRoll` (Channel Rack's per-row "open piano roll" button) also calls `windowFocus.focus('piano-roll')` so it correctly raises the window instead of relying on mount order.

## Piano Roll

**Details: [`docs/piano-roll.md`](docs/piano-roll.md)** (tools, zoom, key highlight, note properties & slides, deletion effect, velocity lane, overview, playhead). Key points:

- Floating window editing `channelStore.selectedChannel`; opened by double-clicking a channel's sample name or from the Toolbar (auto-selects the first channel).
- `pianoroll/pitch.ts` is the single source of pixel math; sizes go through zoom-aware `keyH()`/`stepW()` (`MiniNoteRoll`'s Channel Rack preview uses fixed `BASE_*`). Shift during a drag = unsnapped fractional steps.
- Only `NoteGrid`'s viewport scrolls natively; keys/ruler/velocity lane follow via `transform`. All three viewports need `height: 100%` or nothing clips.
- Tools: Draw (click places/selects + adopts length/color, drag moves, right edge resizes), Marquee, Delete; right button deletes in every tool. Delete with nothing selected clears the channel's notes.
- Note properties (velocity, pan, release, modX/modY filter, fine pitch, palette color, slide) via `noteProps.ts`; `normalizeNote()` on import. Slide notes glide same-channel same-color voices (`slideVoices.ts`).
- Deletion effect (`deleteFx.ts`) only plays for elements marked via `markDeleted(key)` before removal — new delete paths must call it.
- Auto-extend grows `patternLength` by a bar when notes reach the last bar. The playhead is wrapped to the open channel's own loop length.

## Mixer

**Details: [`docs/mixer.md`](docs/mixer.md)** (data model, selection, click-to-route, renaming, mute/solo, layout, FX rack, audio graph, history). Key points:

- `mixerStore.svelte.ts` is **ephemeral** (not saved/undo-tracked) even though it drives real audio. `MASTER_TRACK_ID = 0`; channels have multi-target `routeTo: number[]`, cycle-checked in `toggleRoute()`. Display names come from list position; `id` is a never-reused join key (`ChannelData.mixerTrack`).
- `audioEngine` keeps a persistent `TrackChain` per track (input → pan → mute → fan-out + analyser tap) into `masterVolumeGain` → compressor. `mixerAudio.svelte.ts` bridges store → graph with one `$effect` per concern and **diffs routes** (`lastRoutes`); removing a channel must also disconnect other channels' edges *into* it.
- Strips stretch to window height; the fader/meter height is measured (`bind:clientHeight`) because `Slider`'s drag math depends on its `height` prop. Insert slots are UI-only placeholders (no DSP/plugins).
