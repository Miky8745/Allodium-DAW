<script lang="ts">
  import type { MenuItem } from '$lib/types';
  import MenuBar from './MenuBar.svelte';
  import BlackBox from './BlackBox.svelte';
  import PeakMeter from './PeakMeter.svelte';
  import MidiLed from './midi/MidiLed.svelte';
  import { qwertyKeyboard } from '$lib/midi/qwertyKeyboard.svelte';
  import { playback } from '$lib/playbackStore.svelte';
  import { patternStore } from '$lib/patternStore.svelte';
  import { windowFocus } from '$lib/windowFocusStore.svelte';
  import { hoverInfo } from '$lib/hoverInfoStore.svelte';
  import { hoverInfoAction } from '$lib/hoverInfo';

  let {
    menus,
    showExplorer    = $bindable(true),
    showChannelRack = $bindable(false),
    showPianoRoll   = $bindable(false),
    showPlaylist    = $bindable(true),
    showMixer       = $bindable(false),
  }: {
    menus: { name: string; items: MenuItem[] }[];
    showExplorer?:    boolean;
    showChannelRack?: boolean;
    showPianoRoll?:   boolean;
    showPlaylist?:    boolean;
    showMixer?:       boolean;
  } = $props();

  // Panel toggle buttons: closed -> open+focus; open-but-not-frontmost ->
  // bring to front (stays open); open-and-frontmost -> close. Matches
  // common desktop taskbar semantics.
  function toggleChannelRack() {
    if (!showChannelRack) { showChannelRack = true; windowFocus.focus('channel-rack'); }
    else if (!windowFocus.isFrontmost('channel-rack')) windowFocus.focus('channel-rack');
    else showChannelRack = false;
  }
  function togglePianoRoll() {
    if (!showPianoRoll) { showPianoRoll = true; windowFocus.focus('piano-roll'); }
    else if (!windowFocus.isFrontmost('piano-roll')) windowFocus.focus('piano-roll');
    else showPianoRoll = false;
  }
  function togglePlaylist() {
    if (!showPlaylist) { showPlaylist = true; windowFocus.focus('playlist'); }
    else if (!windowFocus.isFrontmost('playlist')) windowFocus.focus('playlist');
    else showPlaylist = false;
  }
  function toggleMixer() {
    if (!showMixer) { showMixer = true; windowFocus.focus('mixer'); }
    else if (!windowFocus.isFrontmost('mixer')) windowFocus.focus('mixer');
    else showMixer = false;
  }

  // Transport
  let isPaused    = $state(false);
  let isRecording = $state(false);

  // Display — tempo/isPlaying/mode are sourced from the shared playback store
  let position     = $state({ beat: 1, step: 1, tick: 0 });

  // Row-1 toggles
  // "Typing keyboard to piano" lives in qwertyKeyboard.svelte.ts (QwertyPiano
  // listens for keys while it's on).
  let precount       = $state(false);
  let loopRecord     = $state(false);
  // Metronome lives in playbackStore (playback.metronomeOn), not here — the
  // scheduler needs a live getter for it, same as tempo/channels/barLength.

  // Row-2 toggles
  let autoScroll     = $state(false);
  let oneClickRecord = $state(false);

  // Dropdowns
  let snapping = $state('1/4');

  // Modifier key indicators
  let shiftDown = $state(false);
  let altDown   = $state(false);
  let ctrlDown  = $state(false);

  let posDisplay = $derived(
    `${position.beat}:${position.step}:${String(position.tick).padStart(2, '0')}`
  );

  $effect(() => {
    function onDown(e: KeyboardEvent) {
      if (e.key === 'Shift')   shiftDown = true;
      if (e.key === 'Alt')     altDown   = true;
      if (e.key === 'Control') ctrlDown  = true;
    }
    function onUp(e: KeyboardEvent) {
      if (e.key === 'Shift')   shiftDown = false;
      if (e.key === 'Alt')     altDown   = false;
      if (e.key === 'Control') ctrlDown  = false;
    }
    window.addEventListener('keydown', onDown);
    window.addEventListener('keyup',   onUp);
    return () => {
      window.removeEventListener('keydown', onDown);
      window.removeEventListener('keyup',   onUp);
    };
  });

  function play() {
    isPaused = false;
    playback.soloChannelId = null;
    playback.isPlaying = !playback.isPlaying;
  }
  function stop() {
    isPaused = false;
    playback.soloChannelId = null;
    playback.isPlaying = false;
    position = { beat: 1, step: 1, tick: 0 };
  }
  function pause() {
    if (playback.isPlaying)  { playback.isPlaying = false; isPaused = true; }
    else if (isPaused)       { playback.isPlaying = true;  isPaused = false; }
  }

  // Turning the metronome on doubles as pressing play (so you hear it
  // immediately instead of silently arming it) — only when playback isn't
  // already running, and reusing play() itself rather than setting
  // isPlaying directly so it gets the exact same isPaused/soloChannelId
  // reset the real Play button gets. Turning it back off doesn't stop
  // playback — only the on-transition auto-starts.
  function toggleMetronome() {
    playback.metronomeOn = !playback.metronomeOn;
    if (playback.metronomeOn && !playback.isPlaying) play();
  }

  function onTempoWheel(e: WheelEvent) {
    e.preventDefault();
    const step = e.shiftKey ? 10 : 1;
    playback.tempo = Math.max(20, Math.min(999, playback.tempo - Math.sign(e.deltaY) * step));
  }
</script>

<div class="toolbar">

  <!-- ── Row 1 / Col 1 : Menu section (grey-green) ─────────────────── -->
  <div class="tb-menu">
    <MenuBar {menus} />
  </div>

  <!-- ── Row 1 / Col 2 : Transport + display + toggles + monitor ───── -->
  <div class="tb-r1">

    <!-- Pattern / Song mode switch -->
    <div class="tb-vsep"></div>
    <div class="tb-patsng" role="group" aria-label="Playback mode">
      <div class="tb-patsng-pill" style="transform: translateY({playback.transportMode === 'song' ? '100%' : '0'})"></div>
      <button
        class="tb-patsng-btn"
        class:tb-patsng-btn--on={playback.transportMode === 'pattern'}
        onclick={() => playback.transportMode = 'pattern'}
        use:hoverInfoAction={{ name: 'Pattern mode', value: playback.transportMode === 'pattern' ? 'Active' : 'Inactive' }}
      >PAT</button>
      <button
        class="tb-patsng-btn"
        class:tb-patsng-btn--on={playback.transportMode === 'song'}
        onclick={() => playback.transportMode = 'song'}
        use:hoverInfoAction={{ name: 'Song mode', value: playback.transportMode === 'song' ? 'Active' : 'Inactive' }}
      >SONG</button>
    </div>

    <!-- Transport buttons: Play · Stop · Pause · Record -->
    <div class="tb-vsep"></div>
    <div class="tb-transport">
      <button class="tb-xp" class:tb-xp--on={playback.isPlaying} onclick={play} title="Play" use:hoverInfoAction={{ name: 'Play', value: playback.isPlaying ? 'Playing' : 'Stopped' }}>
        <svg width="12" height="13" viewBox="0 0 12 13" fill="currentColor" aria-hidden="true">
          <path d="M1 1 L11 6.5 L1 12 Z"/>
        </svg>
      </button>
      <button class="tb-xp" onclick={stop} title="Stop" use:hoverInfoAction={{ name: 'Stop' }}>
        <svg width="11" height="11" viewBox="0 0 11 11" fill="currentColor" aria-hidden="true">
          <rect x="0.5" y="0.5" width="10" height="10" rx="1.5"/>
        </svg>
      </button>
      <button class="tb-xp" class:tb-xp--on={isPaused} onclick={pause} title="Pause" use:hoverInfoAction={{ name: 'Pause', value: isPaused ? 'Paused' : 'Not paused' }}>
        <svg width="10" height="12" viewBox="0 0 10 12" fill="currentColor" aria-hidden="true">
          <rect x="0" y="0" width="3.5" height="12" rx="1"/>
          <rect x="6.5" y="0" width="3.5" height="12" rx="1"/>
        </svg>
      </button>
      <button class="tb-xp tb-xp--rec" class:tb-xp--on={isRecording} onclick={() => isRecording = !isRecording} title="Record" use:hoverInfoAction={{ name: 'Record', value: isRecording ? 'Recording' : 'Idle' }}>
        <svg width="11" height="11" viewBox="0 0 11 11" fill="currentColor" aria-hidden="true">
          <circle cx="5.5" cy="5.5" r="5"/>
        </svg>
      </button>
    </div>

    <!-- Tempo (scrollable) -->
    <div class="tb-vsep"></div>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="tb-display tb-display--tempo" role="group" aria-label="Tempo" onwheel={onTempoWheel} use:hoverInfoAction={{ name: 'Tempo', value: `${playback.tempo.toFixed(1)} BPM` }}>
      <span class="tb-dlabel">BPM</span>
      <span class="tb-dvalue">{playback.tempo.toFixed(1)}</span>
    </div>

    <!-- Song position -->
    <div class="tb-vsep"></div>
    <div class="tb-display" role="group" aria-label="Song position" use:hoverInfoAction={{ name: 'Song position', value: posDisplay }}>
      <span class="tb-dlabel">POS</span>
      <span class="tb-dvalue">{posDisplay}</span>
    </div>

    <!-- Feature toggles -->
    <div class="tb-vsep"></div>
    <div class="tb-toggles">

      <!-- Typing keyboard → piano -->
      <button class="tb-tgl" class:tb-tgl--on={qwertyKeyboard.enabled} onclick={() => qwertyKeyboard.toggle()} title="Typing keyboard to piano (Z/Q rows, , . = octave)" use:hoverInfoAction={{ name: 'Typing keyboard to piano', value: qwertyKeyboard.enabled ? `On · octave ${qwertyKeyboard.octave >= 0 ? '+' : ''}${qwertyKeyboard.octave}` : 'Off' }}>
        <svg width="20" height="13" viewBox="0 0 20 13" fill="currentColor" aria-hidden="true">
          <!-- keyboard body -->
          <rect x="0" y="1" width="7.5" height="7.5" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.1"/>
          <rect x="1" y="2.5" width="1.8" height="1.8" rx="0.4"/>
          <rect x="3.5" y="2.5" width="1.8" height="1.8" rx="0.4"/>
          <rect x="5.8" y="2.5" width="1" height="1.8" rx="0.4"/>
          <rect x="1" y="5.2" width="5.5" height="1.8" rx="0.4"/>
          <!-- arrow -->
          <path d="M8.8 5 L10.3 5 M9.5 3.8 L10.4 5 L9.5 6.2" stroke-width="1.1" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>
          <!-- piano: 3 white keys + 2 black keys -->
          <rect x="11.5" y="0" width="2.2" height="13" rx="0.6"/>
          <rect x="14.5" y="0" width="2.2" height="13" rx="0.6"/>
          <rect x="17.5" y="0" width="2.2" height="13" rx="0.6"/>
          <rect x="13" y="0" width="1.8" height="7.5" rx="0.4" style="fill: var(--sidebar-bg)"/>
          <rect x="16" y="0" width="1.8" height="7.5" rx="0.4" style="fill: var(--sidebar-bg)"/>
        </svg>
      </button>

      <!-- Precount 3·2·1 -->
      <button class="tb-tgl" class:tb-tgl--on={precount} onclick={() => precount = !precount} title="Recording precount (3–2–1)" use:hoverInfoAction={{ name: 'Recording precount', value: precount ? 'On' : 'Off' }}>
        <span class="tb-321">3·2·1</span>
      </button>

      <!-- Loop record -->
      <button class="tb-tgl" class:tb-tgl--on={loopRecord} onclick={() => loopRecord = !loopRecord} title="Loop recording" use:hoverInfoAction={{ name: 'Loop recording', value: loopRecord ? 'On' : 'Off' }}>
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M12.5 5 A6 6 0 1 0 11 10.5"/>
          <path d="M11 14 L11 10 L14.5 10"/>
          <circle cx="7" cy="7" r="2" fill="currentColor" stroke="none"/>
        </svg>
      </button>

      <!-- Metronome -->
      <button class="tb-tgl" class:tb-tgl--on={playback.metronomeOn} onclick={toggleMetronome} title="Metronome" use:hoverInfoAction={{ name: 'Metronome', value: playback.metronomeOn ? 'On' : 'Off' }}>
        <svg width="12" height="15" viewBox="0 0 12 15" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M1.5 14 L6 1 L10.5 14 Z"/>
          <line x1="6" y1="14" x2="9.5" y2="5"/>
          <circle cx="6" cy="14" r="1" fill="currentColor" stroke="none"/>
        </svg>
      </button>
    </div>

    <!-- Audio monitor: oscilloscope / spectrum analyzer -->
    <div class="tb-vsep"></div>
    <BlackBox />

    <!-- Stereo peak meter -->
    <PeakMeter />
  </div>

  <!-- ── Row 2 / Col 1 : Info panel (auto-matches menu section width) ─
       Shows the name/value of the last-hovered control (hoverInfoStore.
       svelte.ts, written by the hoverInfo action used throughout
       Dial/Knob/ScrollField/Slider and this Toolbar's own controls) —
       stays showing it after the mouse moves off, rather than reverting to
       an idle placeholder; blank until anything's been hovered at all. -->
  <div class="tb-info">
    <span class="tb-info-line">{hoverInfo.name ?? ''}</span>
    <span class="tb-info-line tb-info-muted">{hoverInfo.value ?? ''}</span>
  </div>

  <!-- ── Row 2 / Col 2 : Panel toggles · snap · pattern · key badges ─ -->
  <div class="tb-r2">
    <div class="tb-vsep-sm"></div>

    <!-- Explorer -->
    <button class="tb-pb" class:tb-pb--on={showExplorer} onclick={() => showExplorer = !showExplorer} title="File Explorer" use:hoverInfoAction={{ name: 'File Explorer', value: showExplorer ? 'On' : 'Off' }}>
      <svg width="13" height="12" viewBox="0 0 13 12" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M0.5 2.5 H5 L6.5 4 H12.5 V11 H0.5 Z"/>
        <line x1="0.5" y1="7" x2="12.5" y2="7"/>
      </svg>
    </button>

    <!-- Channel Rack -->
    <button class="tb-pb" class:tb-pb--on={showChannelRack} onclick={toggleChannelRack} title="Channel Rack" use:hoverInfoAction={{ name: 'Channel Rack', value: showChannelRack ? 'On' : 'Off' }}>
      <svg width="13" height="12" viewBox="0 0 13 12" fill="currentColor" aria-hidden="true">
        <rect x="0"    y="2"  width="2" height="10" rx="0.75"/>
        <rect x="2.75" y="0"  width="2" height="12" rx="0.75"/>
        <rect x="5.5"  y="4"  width="2" height="8"  rx="0.75"/>
        <rect x="8.25" y="1"  width="2" height="11" rx="0.75"/>
        <rect x="11"   y="3"  width="2" height="9"  rx="0.75"/>
      </svg>
    </button>

    <!-- Piano Roll -->
    <button class="tb-pb" class:tb-pb--on={showPianoRoll} onclick={togglePianoRoll} title="Piano Roll" use:hoverInfoAction={{ name: 'Piano Roll', value: showPianoRoll ? 'On' : 'Off' }}>
      <svg width="13" height="13" viewBox="0 0 13 13" fill="currentColor" aria-hidden="true">
        <rect x="0"   y="0" width="3.5" height="13" rx="0.75"/>
        <rect x="4.5" y="0" width="3.5" height="13" rx="0.75"/>
        <rect x="9.5" y="0" width="3.5" height="13" rx="0.75"/>
        <rect x="2.3" y="0" width="2.5" height="7.5" rx="0.5" style="fill: var(--main-bg)"/>
        <rect x="7.3" y="0" width="2.5" height="7.5" rx="0.5" style="fill: var(--main-bg)"/>
      </svg>
    </button>

    <!-- Playlist -->
    <button class="tb-pb" class:tb-pb--on={showPlaylist} onclick={togglePlaylist} title="Playlist" use:hoverInfoAction={{ name: 'Playlist', value: showPlaylist ? 'On' : 'Off' }}>
      <svg width="13" height="11" viewBox="0 0 13 11" fill="currentColor" aria-hidden="true">
        <rect x="0" y="0"    width="13" height="2.5" rx="0.75"/>
        <rect x="0" y="4.25" width="13" height="2.5" rx="0.75"/>
        <rect x="0" y="8.5"  width="13" height="2.5" rx="0.75"/>
      </svg>
    </button>

    <!-- Mixer -->
    <button class="tb-pb" class:tb-pb--on={showMixer} onclick={toggleMixer} title="Mixer" use:hoverInfoAction={{ name: 'Mixer', value: showMixer ? 'On' : 'Off' }}>
      <svg width="13" height="13" viewBox="0 0 13 13" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" aria-hidden="true">
        <line x1="2"   y1="0" x2="2"   y2="13"/>
        <circle cx="2"   cy="4" r="1.4" fill="currentColor" stroke="none"/>
        <line x1="6.5" y1="0" x2="6.5" y2="13"/>
        <circle cx="6.5" cy="8" r="1.4" fill="currentColor" stroke="none"/>
        <line x1="11"  y1="0" x2="11"  y2="13"/>
        <circle cx="11"  cy="5" r="1.4" fill="currentColor" stroke="none"/>
      </svg>
    </button>

    <!-- Auto scroll -->
    <button class="tb-pb" class:tb-pb--on={autoScroll} onclick={() => autoScroll = !autoScroll} title="Auto scroll" use:hoverInfoAction={{ name: 'Auto scroll', value: autoScroll ? 'On' : 'Off' }}>
      <svg width="13" height="11" viewBox="0 0 13 11" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <line x1="0" y1="1.5" x2="9"  y2="1.5"/>
        <line x1="0" y1="5.5" x2="9"  y2="5.5"/>
        <line x1="0" y1="9.5" x2="9"  y2="9.5"/>
        <path d="M8.5 0 L13 5.5 L8.5 11"/>
      </svg>
    </button>

    <!-- One-click audio recording (mic) -->
    <button class="tb-pb" class:tb-pb--on={oneClickRecord} onclick={() => oneClickRecord = !oneClickRecord} title="One-click audio recording" use:hoverInfoAction={{ name: 'One-click audio recording', value: oneClickRecord ? 'On' : 'Off' }}>
      <svg width="10" height="14" viewBox="0 0 10 14" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <rect x="2" y="0.5" width="6" height="8" rx="3"/>
        <path d="M0.5 7.5 A4.5 4.5 0 0 0 9.5 7.5"/>
        <line x1="5" y1="12" x2="5" y2="10.5"/>
        <line x1="2.5" y1="13.5" x2="7.5" y2="13.5"/>
      </svg>
    </button>

    <div class="tb-vsep-sm"></div>

    <!-- Snapping -->
    <div class="tb-dd" use:hoverInfoAction={{ name: 'Snapping', value: snapping }}>
      <select class="tb-sel" bind:value={snapping} title="Snapping">
        <option value="1/4">1/4</option>
        <option value="1/2">1/2</option>
      </select>
      <svg class="tb-dd-arrow" width="8" height="5" viewBox="0 0 8 5" fill="none" aria-hidden="true">
        <path d="M1 1 L4 4 L7 1" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
    </div>

    <!-- Pattern selection -->
    <div class="tb-dd" use:hoverInfoAction={{ name: 'Pattern', value: patternStore.selectedPattern?.name ?? '' }}>
      <select class="tb-sel" bind:value={patternStore.selectedPatternId} title="Pattern">
        {#each patternStore.patterns as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
      <svg class="tb-dd-arrow" width="8" height="5" viewBox="0 0 8 5" fill="none" aria-hidden="true">
        <path d="M1 1 L4 4 L7 1" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
    </div>

    <div class="tb-spacer"></div>

    <MidiLed />

    <!-- Modifier key indicators -->
    <div class="tb-keys">
      <span class="tb-key" class:tb-key--on={shiftDown} use:hoverInfoAction={{ name: 'Shift', value: shiftDown ? 'Held' : 'Not held' }}>SHF</span>
      <span class="tb-key" class:tb-key--on={altDown} use:hoverInfoAction={{ name: 'Alt', value: altDown ? 'Held' : 'Not held' }}>ALT</span>
      <span class="tb-key" class:tb-key--on={ctrlDown} use:hoverInfoAction={{ name: 'Ctrl', value: ctrlDown ? 'Held' : 'Not held' }}>CTL</span>
    </div>
  </div>
</div>

<style>
  /* ── Grid wrapper ───────────────────────────────────────────── */
  .toolbar {
    display: grid;
    grid-template-columns: max-content 1fr;
    grid-template-rows: 50px 50px;
    background: var(--sidebar-bg);
    border-bottom: 1px solid var(--sidebar-border);
    flex-shrink: 0;
    user-select: none;
    overflow: hidden;
  }

  /* ── Row 1 / Col 1 : Menu section ──────────────────────────── */
  .tb-menu {
    grid-row: 1;
    grid-column: 1;
    display: flex;
    align-items: center;
    padding: 0 10px;
    background: var(--toolbar-green-bg);
    border-right: 1px solid rgba(0, 0, 0, 0.35);
  }

  /* ── Row 1 / Col 2 : Transport + controls ───────────────────── */
  .tb-r1 {
    grid-row: 1;
    grid-column: 2;
    display: flex;
    align-items: center;
  }

  /* ── Row 2 / Col 1 : Info box ───────────────────────────────── */
  .tb-info {
    grid-row: 2;
    grid-column: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 1px;
    padding: 2px 8px;
    border-top: 1px solid var(--sidebar-border);
    border-right: 1px solid rgba(0, 0, 0, 0.35);
  }

  .tb-info-line {
    font-size: 10px;
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    color: var(--main-text);
    line-height: 1.3;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tb-info-muted { color: var(--main-text-muted); }

  /* ── Row 2 / Col 2 : Panel toggles + dropdowns + keys ────────── */
  .tb-r2 {
    grid-row: 2;
    grid-column: 2;
    display: flex;
    align-items: center;
    border-top: 1px solid var(--sidebar-border);
    gap: 1px;
    padding: 0 4px;
  }

  /* ── Separators ─────────────────────────────────────────────── */
  .tb-vsep {
    width: 1px;
    height: 28px;
    background: rgba(255, 255, 255, 0.07);
    margin: 0 8px;
    flex-shrink: 0;
  }
  .tb-vsep-sm {
    width: 1px;
    height: 16px;
    background: rgba(255, 255, 255, 0.07);
    margin: 0 5px;
    flex-shrink: 0;
  }

  /* ── Pattern / Song switch ───────────────────────────────────── */
  .tb-patsng {
    position: relative;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--sidebar-border);
    border-radius: 7px;
    overflow: hidden;
  }
  .tb-patsng-pill {
    position: absolute;
    inset: 0;
    height: 50%;
    background: var(--accent);
    border-radius: 5px;
    transition: transform 0.18s cubic-bezier(0.4, 0, 0.2, 1);
    z-index: 0;
  }
  .tb-patsng-btn {
    position: relative;
    z-index: 1;
    background: transparent;
    border: none;
    color: var(--btn-text);
    font-family: inherit;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.05em;
    padding: 3px 9px;
    cursor: pointer;
    transition: color 0.15s;
    line-height: 1;
  }
  .tb-patsng-btn--on {
    color: #e4e5e6;
  }
  .tb-patsng-btn:not(.tb-patsng-btn--on):hover {
    color: var(--main-text);
  }

  /* ── Transport buttons ───────────────────────────────────────── */
  .tb-transport { display: flex; align-items: center; gap: 3px; }

  .tb-xp {
    width: 34px;
    height: 34px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--btn-bg);
    border: 1px solid var(--btn-border);
    border-radius: 8px;
    color: var(--btn-text);
    cursor: pointer;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
  }
  .tb-xp:hover { background: var(--btn-hover); color: var(--main-text); }
  .tb-xp--on {
    border-color: var(--accent);
    color: var(--accent);
    background: rgba(230,176,127, 0.09);
  }
  .tb-xp--rec { color: #b85151; }
  .tb-xp--rec.tb-xp--on {
    color: #d26868;
    border-color: #b85151;
    background: rgba(196, 59, 59, 0.12);
  }

  /* ── Display blocks (BPM / position, grey-green bg) ────────── */
  .tb-display {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    background: var(--toolbar-green-bg);
    padding: 5px 12px;
    height: 100%;
    gap: 2px;
    cursor: default;
  }
  .tb-display--tempo { cursor: ns-resize; }

  .tb-dlabel {
    font-size: 8.5px;
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    color: rgba(160, 210, 175, 0.55);
    letter-spacing: 0.1em;
    line-height: 1;
  }
  .tb-dvalue {
    font-size: 16px;
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    color: #f3cfae;
    line-height: 1;
    letter-spacing: 0.02em;
  }

  /* ── Feature toggles (row 1 right cluster) ───────────────────── */
  .tb-toggles { display: flex; align-items: center; gap: 2px; }

  .tb-tgl {
    width: 32px;
    height: 34px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    color: var(--btn-text);
    cursor: pointer;
    border-radius: 7px;
    transition: background 0.1s, color 0.1s;
  }
  .tb-tgl:hover { background: rgba(255, 255, 255, 0.05); color: var(--main-text); }
  .tb-tgl--on {
    color: var(--accent);
    background: rgba(230,176,127, 0.08);
  }

  .tb-321 {
    font-size: 8.5px;
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-weight: 700;
    letter-spacing: 0.01em;
    line-height: 1;
    pointer-events: none;
  }

  /* ── Row-2 panel buttons ─────────────────────────────────────── */
  .tb-pb {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    background: transparent;
    border: none;
    color: var(--btn-text);
    cursor: pointer;
    border-radius: 5px;
    transition: background 0.1s, color 0.1s;
    flex-shrink: 0;
  }
  .tb-pb:hover { background: rgba(255, 255, 255, 0.05); color: var(--main-text); }
  .tb-pb--on {
    color: var(--accent);
    background: rgba(230,176,127, 0.08);
  }

  /* ── Dropdowns ────────────────────────────────────────────────── */
  .tb-dd {
    position: relative;
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }
  .tb-sel {
    appearance: none;
    -webkit-appearance: none;
    background: var(--btn-bg);
    border: 1px solid var(--btn-border);
    color: var(--btn-text);
    font-family: inherit;
    font-size: 11px;
    padding: 4px 20px 4px 8px;
    border-radius: 7px;
    cursor: pointer;
    height: 28px;
    outline: none;
    flex-shrink: 0;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
  }
  .tb-sel:hover { background: var(--btn-hover); color: var(--main-text); }
  .tb-sel:focus { border-color: var(--accent); color: var(--main-text); }
  .tb-dd-arrow {
    position: absolute;
    right: 7px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--main-text-muted);
    pointer-events: none;
    transition: color 0.1s;
  }
  .tb-dd:hover .tb-dd-arrow,
  .tb-sel:focus + .tb-dd-arrow { color: var(--accent); }

  /* ── Spacer + key indicators ─────────────────────────────────── */
  .tb-spacer { flex: 1; }

  .tb-keys {
    display: flex;
    gap: 3px;
    padding: 0 8px;
  }
  .tb-key {
    font-size: 9px;
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-weight: 700;
    letter-spacing: 0.04em;
    padding: 4px 7px;
    border-radius: 7px;
    background: var(--btn-bg);
    border: 1px solid var(--btn-border);
    color: rgba(255, 255, 255, 0.18);
    transition: background 0.1s, color 0.1s, border-color 0.1s;
    line-height: 1.4;
  }
  .tb-key--on {
    background: rgba(230,176,127, 0.14);
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
