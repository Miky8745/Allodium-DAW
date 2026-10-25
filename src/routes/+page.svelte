<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { LogicalSize } from '@tauri-apps/api/dpi';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import ProjectsPanel from '$lib/components/ProjectsPanel.svelte';
  import NewProjectModal from '$lib/components/NewProjectModal.svelte';
  import Toolbar from '$lib/components/Toolbar.svelte';
  import FileExplorer from '$lib/components/FileExplorer.svelte';
  import Playlist from '$lib/components/Playlist.svelte';
  import ChannelRack from '$lib/components/ChannelRack.svelte';
  import PianoRoll from '$lib/components/PianoRoll.svelte';
  import Mixer from '$lib/components/mixer/Mixer.svelte';
  import InstrumentWindow from '$lib/components/instrument/InstrumentWindow.svelte';
  import ProjectShortcuts from '$lib/components/ProjectShortcuts.svelte';
  import PreferencesModal from '$lib/components/preferences/PreferencesModal.svelte';
  import QwertyPiano from '$lib/components/midi/QwertyPiano.svelte';
  import { preferences } from '$lib/preferencesStore.svelte';
  import { preferencesUi } from '$lib/preferencesUi.svelte';
  import { startApplyingPreferences } from '$lib/preferencesApply.svelte';
  import { midiSystem } from '$lib/midi/midiSystem.svelte';
  import { liveNotes } from '$lib/midi/liveNotes.svelte';
  import { qwertyKeyboard } from '$lib/midi/qwertyKeyboard.svelte';
  import { channelStore } from '$lib/channelStore.svelte';
  import { historyStore } from '$lib/historyStore.svelte';
  import { playback } from '$lib/playbackStore.svelte';
  import { mixerAudio } from '$lib/mixerAudio.svelte';
  import { audioEngine } from '$lib/audioEngine';
  import { windowFocus } from '$lib/windowFocusStore.svelte';
  import { saveProject, loadProject, resetAllStores } from '$lib/projectSerializer';
  import type { FileNode } from '$lib/types';
  import { runDebugHarness } from '$lib/debug/autoplay';

  type View = 'home' | 'project';
  type NavKey = 'projects' | 'customize' | 'plugins' | 'learn';

  let view = $state<View>('home');
  let activeNav = $state<NavKey>('projects');
  let projects = $state<string[]>([]);
  let showModal = $state(false);
  let projectName = $state('');
  let fileTree = $state<FileNode[]>([]);
  let showExplorer    = $state(true);
  let showPlaylist    = $state(true);
  let showChannelRack = $state(false);
  let showPianoRoll   = $state(false);
  let showMixer       = $state(false);
  let showPianoRollOverview = $state(false);

  // The zone to the right of the file explorer and below the toolbar — this
  // is what "maximize" fills for the Playlist/ChannelRack/PianoRoll floating
  // windows. Measured off the empty `.workspace-zone` placeholder div so it
  // stays correct as the explorer is toggled or the window is resized.
  let workspaceEl = $state<HTMLDivElement | undefined>(undefined);
  let workspaceBounds = $state({ x: 0, y: 0, width: 0, height: 0 });

  function measureWorkspace() {
    if (!workspaceEl) return;
    const r = workspaceEl.getBoundingClientRect();
    workspaceBounds = { x: r.left, y: r.top, width: r.width, height: r.height };
  }

  $effect(() => {
    if (view !== 'project' || !workspaceEl) return;
    measureWorkspace();
    const ro = new ResizeObserver(measureWorkspace);
    ro.observe(workspaceEl);
    window.addEventListener('resize', measureWorkspace);
    return () => {
      ro.disconnect();
      window.removeEventListener('resize', measureWorkspace);
    };
  });

  async function openProject(name: string) {
    const win = getCurrentWindow();
    try {
      await win.setTitle(name);
      await win.maximize();
      // Background dev launch (ALLODIUM_BACKGROUND, see lib.rs): keep the
      // window out of the way — it's still maximized once restored.
      if (await invoke<boolean>('background_launch')) await win.minimize();
    } catch (_) {}
    fileTree = await invoke<FileNode[]>('list_data_files');
    projectName = name;

    const loaded = await loadProject(name);
    if (!loaded) {
      resetAllStores();
      // Brand-new (never saved) project: Preferences > General defaults.
      playback.tempo = preferences.general.defaultBpm;
      channelStore.beatsPerBar = preferences.general.defaultBeatsPerBar;
    }
    preferences.general.lastProject = name;
    liveNotes.active = true;
    audioEngine.warmUp();
    audioEngine.preloadChannels(channelStore.channels);
    audioEngine.preloadMetronome();
    await historyStore.init(name);
    historyStore.startWatching();
    playback.startWatching();
    mixerAudio.startWatching();

    view = 'project';
  }

  async function exitProject() {
    liveNotes.active = false;
    liveNotes.releaseAll();
    qwertyKeyboard.releaseAll();
    historyStore.stopWatching();
    playback.stopWatching();
    mixerAudio.stopWatching();
    const win = getCurrentWindow();
    try {
      await win.unmaximize();
      await win.setSize(new LogicalSize(800, 600));
      await win.center();
      await win.setTitle('Allodium');
    } catch (_) {}
    view = 'home';
  }

  let menus = $derived([
    { name: 'File', items: [
      { label: 'Save', action: () => saveProject(projectName) },
      { label: historyStore.autosaveEnabled ? 'Autosave: On' : 'Autosave: Off',
        action: () => { historyStore.autosaveEnabled = !historyStore.autosaveEnabled; } },
      { label: 'Exit project', action: exitProject },
    ] },
    { name: 'Edit',     items: [
      { label: 'Preferences…', action: () => preferencesUi.show() },
    ] },
    { name: 'Add',      items: [] },
    { name: 'Patterns', items: [] },
    { name: 'View',     items: [] },
    { name: 'Options',  items: [] },
    { name: 'Tools',    items: [] },
    { name: 'Help',     items: [] },
  ]);

  onMount(async () => {
    await preferences.load();
    preferences.startWatching();
    startApplyingPreferences();
    void midiSystem.start();
    projects = await invoke<string[]>('list_projects');
    const last = preferences.general.lastProject;
    if (preferences.general.reopenLastProject && last && projects.includes(last)) {
      await openProject(last);
    }
    // Dev-only profiling/autoplay (no-op unless ALLODIUM_PROFILE /
    // ALLODIUM_AUTOPLAY is set — see src-tauri/src/debug.rs).
    void runDebugHarness(openProject, () => (view === 'project' ? projectName : ''), (names) => {
      showChannelRack = names.includes('channelrack');
      showPianoRoll = names.includes('pianoroll');
      showMixer = names.includes('mixer');
      showPianoRollOverview = names.includes('overview');
      showPlaylist = !names.includes('noplaylist');
    });
  });

  // Ctrl+, opens Preferences from either view.
  function onGlobalKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key === ',') {
      e.preventDefault();
      preferencesUi.show();
    }
  }
</script>

<svelte:window onkeydown={onGlobalKeydown} />
<PreferencesModal />

{#if view === 'home'}
  <div class="app">
    <Sidebar bind:activeNav />
    <main class="main">
      {#if activeNav === 'projects'}
        <ProjectsPanel {projects} onNewProject={() => showModal = true} onOpenProject={openProject} />
      {:else}
        <div class="placeholder-panel">
          <div class="placeholder-content">
            <svg width="52" height="52" viewBox="0 0 52 52" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" opacity="0.2" aria-hidden="true">
              <rect x="4" y="4" width="44" height="44" rx="10"/>
              <path d="M18 26h16M26 18v16"/>
            </svg>
            <p class="coming-soon-label">Coming Soon</p>
          </div>
        </div>
      {/if}
    </main>
    <NewProjectModal bind:show={showModal} onCreated={openProject} />
  </div>
{:else}
  <ProjectShortcuts />
  <QwertyPiano />
  <div class="workspace">
    <Toolbar
      {menus}
      bind:showExplorer
      bind:showChannelRack
      bind:showPianoRoll
      bind:showPlaylist
      bind:showMixer
    />

    <!-- Body: left panel + workspace -->
    <div class="body">
      <FileExplorer {fileTree} bind:show={showExplorer} />
      <div class="workspace-zone" bind:this={workspaceEl}></div>
    </div>

    <Playlist bind:show={showPlaylist} {workspaceBounds} />
    <ChannelRack
      bind:show={showChannelRack}
      {workspaceBounds}
      bind:showPianoRollOverview
      onOpenPianoRoll={(channelId) => {
        channelStore.selectedChannelId = channelId;
        showPianoRoll = true;
        windowFocus.focus('piano-roll');
      }}
    />
    <PianoRoll bind:show={showPianoRoll} {workspaceBounds} bind:showPianoRollOverview />
    <Mixer bind:show={showMixer} {workspaceBounds} />
    <InstrumentWindow {workspaceBounds} />
  </div>
{/if}

<style>
  /* ── Home view ─────────────────────────────── */
  .app {
    display: flex;
    height: 100vh;
    font-family: 'DM Sans', 'Segoe UI', system-ui, sans-serif;
    font-size: 14px;
    -webkit-font-smoothing: antialiased;
  }

  .placeholder-panel {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--main-text-muted);
  }

  .placeholder-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }

  .coming-soon-label {
    font-size: 12px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--main-text-muted);
  }

  /* ── Project view ──────────────────────────── */
  .workspace {
    display: flex;
    flex-direction: column;
    height: 100vh;
    font-family: 'DM Sans', 'Segoe UI', system-ui, sans-serif;
    font-size: 13px;
    -webkit-font-smoothing: antialiased;
  }

  /* Body: file explorer + workspace side by side */
  .body { display: flex; flex: 1; min-height: 0; overflow: hidden; }

  /* Empty placeholder whose rect defines where the Playlist/ChannelRack/
     PianoRoll floating windows land when maximized — those windows render
     as fixed-position siblings of `.body`, not inside this div. */
  .workspace-zone {
    flex: 1;
    background: var(--main-bg);
    overflow: hidden;
    min-width: 0;
  }

  /* ── Shared ────────────────────────────────── */
  .main {
    flex: 1;
    background: var(--main-bg);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
  }
</style>
