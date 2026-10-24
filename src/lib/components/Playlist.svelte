<script lang="ts">
  import FloatingWindow, { type WorkspaceBounds } from './FloatingWindow.svelte';
  import PlaylistTrackRow, { type Track } from './playlist/PlaylistTrackRow.svelte';
  import PlaylistOverview from './playlist/PlaylistOverview.svelte';
  import PatternsPanel from './playlist/PatternsPanel.svelte';
  import EditToolButtons from './EditToolButtons.svelte';
  import { PlaylistEditor, freeStart } from './playlist/playlistEditor.svelte';
  import { patternStore } from '$lib/patternStore.svelte';
  import { channelStore } from '$lib/channelStore.svelte';
  import { placementStore } from '$lib/placementStore.svelte';
  import { playback } from '$lib/playbackStore.svelte';
  import { windowFocus } from '$lib/windowFocusStore.svelte';
  import { audioEngine } from '$lib/audioEngine';

  const TRACK_PANEL_W = 180;
  const PATTERNS_PANEL_W = 168;
  const RULER_H = 36; // the song overview bar above the scroller uses the same height
  const CLIP_FALLBACK_LENGTH_BEATS = 4;

  let { show = $bindable(true), workspaceBounds }: { show?: boolean; workspaceBounds: WorkspaceBounds } = $props();

  function close() { show = false; }

  let trackHeight = $state(80);
  let beatWidth = $state(13);
  let barCount = $state(64);

  // Window-level shortcuts, only while the Playlist is the open, frontmost
  // window — otherwise they'd hijack keys meant for the Piano Roll /
  // Channel Rack. Space = play/pause; Delete/Backspace removes the selected
  // blocks; Ctrl+A selects every block; Escape clears the selection.
  function handleKeydown(e: KeyboardEvent) {
    if (!show || !windowFocus.isFrontmost('playlist')) return;
    const t = e.target as HTMLElement | null;
    if (t?.tagName === 'INPUT' || t?.tagName === 'TEXTAREA' || t?.tagName === 'SELECT' || t?.isContentEditable) return;
    if (e.code === 'Space') {
      if (e.repeat) return;
      e.preventDefault(); // also stops a focused button from being "clicked" by Space
      playback.togglePlay();
    } else if (e.key === 'Delete' || e.key === 'Backspace') {
      if (editor.selected.size === 0) return;
      e.preventDefault();
      editor.deleteSelected();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
      e.preventDefault();
      editor.selectAll();
    } else if (e.key === 'Escape') {
      editor.selected.clear();
    }
  }

  function handleGridWheel(e: WheelEvent) {
    if (!e.ctrlKey) return;
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.1 : 1 / 1.1;
    beatWidth = Math.max(10, Math.min(200, Math.round(beatWidth * factor)));
  }

  let tracks = $state<Track[]>([
    { id: 1, name: 'Track 1', color: '#e6b07f' },
    { id: 2, name: 'Track 2', color: '#e6b07f' },
    { id: 3, name: 'Track 3', color: '#e6b07f' },
    { id: 4, name: 'Track 4', color: '#e6b07f' },
    { id: 5, name: 'Track 5', color: '#e6b07f' },
    { id: 6, name: 'Track 6', color: '#e6b07f' },
    { id: 7, name: 'Track 7', color: '#e6b07f' },
    { id: 8, name: 'Track 8', color: '#e6b07f' },
  ]);

  // The grid's cell resolution is one beat (not one bar) — barCount is how
  // many bars to render, beatCount is the actual cell count. This is what
  // makes the timeline 4x finer/wider than a bar-per-cell grid at the same
  // per-cell pixel width.
  let beatCount = $derived(barCount * channelStore.beatsPerBar);
  let totalWidth = $derived(TRACK_PANEL_W + beatCount * beatWidth);
  let pixelsPerBeat = $derived(beatWidth);

  const RULER_A = '#1a1d26';
  const RULER_B = '#1e212a';

  function rulerBg(groupIndex: number): string {
    return groupIndex % 2 === 0 ? RULER_A : RULER_B;
  }

  // ── Pattern/clip placements (drag from the patterns panel, or drop a
  //    file from FileExplorer, onto the timeline) ──────────────────────
  let gridEl = $state<HTMLDivElement | undefined>();

  // Horizontal scroll state of the timeline scroller, mirrored for the song
  // overview bar (PlaylistOverview), which can also drive it.
  let scrollerEl = $state<HTMLDivElement | undefined>();
  let scrollLeft = $state(0);
  let scrollerWidth = $state(0);
  let timelineViewportW = $derived(Math.max(0, scrollerWidth - TRACK_PANEL_W));

  function scrollTimelineTo(px: number) {
    if (scrollerEl) scrollerEl.scrollLeft = px; // fires onscroll → scrollLeft
  }

  // Tools, block selection, moving, marquee and drag-deleting — see
  // playlist/playlistEditor.svelte.ts.
  const editor = new PlaylistEditor({
    grid: () => gridEl,
    pixelsPerBeat: () => pixelsPerBeat,
    trackIds: () => tracks.map(t => t.id),
    trackPanelW: TRACK_PANEL_W,
  });

  $effect(() => editor.prune());

  // Dropping a sample file from FileExplorer creates the clip immediately
  // at a fallback width (so it appears the instant you let go), then
  // corrects lengthBeats once the
  // sample's real decoded duration is known — re-fitting its start if the
  // real length would now run into a neighbour.
  async function onClipDrop(trackId: number, startBeat: number, samplePath: string) {
    const start = freeStart(trackId, startBeat, CLIP_FALLBACK_LENGTH_BEATS, null);
    if (start === null) return;
    const placement = placementStore.addClip(samplePath, trackId, start, CLIP_FALLBACK_LENGTH_BEATS);
    const buf = await audioEngine.loadSample(samplePath);
    if (buf) {
      const lengthBeats = Math.max(0.05, buf.duration / (60 / playback.tempo));
      const current = placementStore.placements.find(p => p.id === placement.id);
      if (!current) return; // deleted/undone meanwhile
      const fitted = freeStart(current.trackId, current.startBeat, lengthBeats, current.id) ?? current.startBeat;
      placementStore.update(placement.id, { lengthBeats, startBeat: fitted });
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} onpointermove={(e) => editor.onWindowPointerMove(e)} onpointerup={() => editor.onWindowPointerUp()} />

<FloatingWindow
  id="playlist"
  bind:show
  {workspaceBounds}
  maximized={true}
  x={100} y={100} width={900} height={500}
  minWidth={480} minHeight={260}
>
  {#snippet header({ onDragStart, maximized, toggleMaximize })}
    <div class="pl-header" onmousedown={onDragStart} role="toolbar" aria-label="Playlist controls" tabindex="-1">
      <div class="pl-header-left">
        <EditToolButtons bind:tool={editor.tool} items="patterns" />
      </div>
      <span class="pl-title">Playlist</span>
      <div class="pl-header-right">
        <button
          class="hdr-btn"
          onclick={(e) => { e.stopPropagation(); toggleMaximize(); }}
          aria-label={maximized ? 'Restore' : 'Maximize'}
          title={maximized ? 'Restore' : 'Maximize'}
        >
          {#if maximized}
            <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
              <rect x="4" y="4" width="8" height="8" rx="1"/>
              <path d="M2 8V3a1 1 0 0 1 1-1h5"/>
            </svg>
          {:else}
            <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
              <rect x="2" y="2" width="10" height="10" rx="1"/>
            </svg>
          {/if}
        </button>
        <button class="close-btn" onclick={close} aria-label="Close">
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <line x1="3" y1="3" x2="11" y2="11"/>
            <line x1="11" y1="3" x2="3" y2="11"/>
          </svg>
        </button>
      </div>
    </div>
  {/snippet}

<div class="playlist-body" style="--panel-w: {TRACK_PANEL_W}px; --patterns-w: {PATTERNS_PANEL_W}px;">
  <PatternsPanel />

  <div class="timeline-col" style="--ruler-h: {RULER_H}px; --panel-w: {TRACK_PANEL_W}px;">
  <!-- Song overview: the whole arrangement squeezed into one bar, with a
       draggable window showing (and moving) what's in view below. -->
  <div class="overview-row">
    <div class="overview-corner">Song</div>
    <PlaylistOverview
      {tracks}
      {beatWidth}
      {beatCount}
      {scrollLeft}
      viewportWidth={timelineViewportW}
      onScrollTo={scrollTimelineTo}
    />
  </div>
  <div
    class="playlist"
    id="playlist-grid"
    style="--track-h: {trackHeight}px; --beat-w: {beatWidth}px; --ruler-h: {RULER_H}px; --panel-w: {TRACK_PANEL_W}px;"
    onwheel={handleGridWheel}
    bind:this={scrollerEl}
    bind:clientWidth={scrollerWidth}
    onscroll={(e) => scrollLeft = (e.currentTarget as HTMLElement).scrollLeft}
  >
    <div class="grid" bind:this={gridEl} style="width: {totalWidth}px;">
      <!-- Ruler row: click sets the Song-mode start marker (playback.seekStep), and also jumps the live playhead while playing -->
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div
        class="ruler-row"
        onclick={(e) => {
          const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
          const beat = Math.max(0, (e.clientX - rect.left - TRACK_PANEL_W) / pixelsPerBeat);
          const step = Math.round(beat * 4);
          // Always moves the start marker; while playing, the live playhead
          // jumps there too.
          playback.seekStep = step;
          playback.jumpTo(step);
        }}
      >
        <div class="ruler-corner"></div>
        {#each Array.from({ length: barCount }) as _, gi}
          <div
            class="ruler-group"
            style="background: {rulerBg(gi)}; width: {channelStore.beatsPerBar * beatWidth}px;"
          >
            <span class="bar-num">{gi + 1}</span>
          </div>
        {/each}
      </div>

      <!-- Track rows -->
      {#each tracks as track (track.id)}
        <PlaylistTrackRow
          {track}
          {beatCount}
          {pixelsPerBeat}
          trackPanelW={TRACK_PANEL_W}
          {gridEl}
          tool={editor.tool}
          movingIds={editor.movingIds}
          selectedIds={editor.selected}
          onLanePointerDown={(e, trackId) => editor.onLanePointerDown(e, trackId)}
          onPlacementPointerDown={(e, placement) => editor.onPlacementPointerDown(e, placement)}
          onDeletePointerDown={(e) => editor.onDeletePointerDown(e)}
          {onClipDrop}
        />
      {/each}

 {#if editor.marquee}
        {@const m = editor.marquee}
        <div
          class="marquee"
          style="left: {Math.min(m.x0, m.x1)}px; top: {Math.min(m.y0, m.y1)}px; width: {Math.abs(m.x1 - m.x0)}px; height: {Math.abs(m.y1 - m.y0)}px;"
        ></div>
      {/if}
      <div class="seek-arrow" style="left: {TRACK_PANEL_W + (playback.seekStep / 4) * pixelsPerBeat}px;"></div>
    </div>
  </div>
  <!-- Continuous song-mode playhead, mirroring the Piano Roll's own
       .playhead-line (NoteGrid.svelte) — meaningless in pattern mode, where
       the timeline position has no relationship to what's playing, so it's
       only shown while actually playing a song. Deliberately drawn in an
       overlay *outside* the scroller (offset by scrollLeft, clipped to the
       lanes), moved with `transform`: inside the 3500px-wide scrolled grid,
       every frame's move made WebKit re-render the grid and any floating
       window stacked over the Playlist — ~25fps and 100ms+ main-thread
       stalls (past the audio scheduler's look-ahead) once the playhead
       reached a window, i.e. "lags after ~4 bars". -->
  {#if playback.isPlaying && playback.transportMode === 'song'}
    <div class="playhead-clip" style="top: {RULER_H}px; left: {TRACK_PANEL_W}px;">
      <div class="playhead" style="transform: translateX({(playback.currentStepFraction / 4) * pixelsPerBeat - scrollLeft}px);">
        <div class="playhead-line"></div>
        <div class="playhead-arrow"></div>
      </div>
    </div>
  {/if}
  </div>

</div>
</FloatingWindow>

<style>
  /* ── Header (mirrors ChannelRack/PianoRollHeader chrome) ──────────── */
  .pl-header {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px;
    height: 36px;
    background: #1d1f29;
    border-bottom: 1px solid var(--explorer-border, #383c4a);
    flex-shrink: 0;
    user-select: none;
  }

  .pl-title {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    font-family: 'Playfair Display', Georgia, serif;
    font-size: 13px;
    font-weight: 600;
    color: var(--main-text, #bcbcc0);
    pointer-events: none;
    white-space: nowrap;
  }

  .pl-header-left {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .marquee {
    position: absolute;
    border: 1px dashed var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    border-radius: 3px;
    pointer-events: none;
    z-index: 3;
  }

  .pl-header-right {
    display: flex;
    align-items: center;
    gap: 2px;
    margin-left: auto;
  }

  .hdr-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    color: rgba(255,255,255,0.45);
    cursor: pointer;
    padding: 0;
    transition: color 0.1s, background 0.1s, border-color 0.1s;
  }

  .hdr-btn svg { width: 14px; height: 14px; }

  .hdr-btn:hover {
    background: rgba(255,255,255,0.07);
    color: rgba(255,255,255,0.75);
  }

  .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    color: rgba(255,255,255,0.4);
    cursor: pointer;
    padding: 0;
    transition: color 0.1s, background 0.1s;
  }

  .close-btn svg { width: 12px; height: 12px; }

  .close-btn:hover {
    background: rgba(220,50,50,0.18);
    color: #f08484;
    border-color: rgba(220,50,50,0.3);
  }

  .playlist-body {
    flex: 1;
    min-height: 0;
    display: flex;
    position: relative;
  }

  .timeline-col {
    position: relative; /* anchors the playhead overlay */
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .overview-row {
    display: flex;
    flex-shrink: 0;
    height: var(--ruler-h);
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .overview-corner {
    width: var(--panel-w);
    flex-shrink: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    padding-left: 10px;
    background: #181b25;
    border-right: 1px solid var(--main-border);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--main-text-muted);
    user-select: none;
  }

  .playlist {
    flex: 1;
    min-height: 0;
    min-width: 0;
    overflow: auto;
  }

  .grid {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    position: relative;
  }

  .playhead-clip {
    position: absolute;
    right: 0;
    bottom: 0;
    overflow: hidden;
    pointer-events: none;
    z-index: 5;
  }

  .playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 0;
    will-change: transform; /* own layer: moving it never repaints the grid */
    pointer-events: none;
  }

  .playhead-line {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    background: #e4e5e6;
    box-shadow: 0 0 4px rgba(255, 255, 255, 0.6);
  }

  .playhead-arrow {
    position: absolute;
    top: 0;
    width: 0;
    height: 0;
    border-left: 6px solid transparent;
    border-right: 6px solid transparent;
    border-top: 10px solid var(--accent);
    filter: drop-shadow(0 0 2px color-mix(in srgb, var(--accent) 60%, transparent));
    margin-left: -6px;
  }

  .seek-arrow {
    position: absolute;
    top: 0;
    width: 0;
    height: 0;
    border-left: 6px solid transparent;
    border-right: 6px solid transparent;
    border-top: 10px solid var(--accent);
    pointer-events: none;
    z-index: 4;
    filter: drop-shadow(0 0 3px color-mix(in srgb, var(--accent) 50%, transparent));
    margin-left: -6px;
  }

  /* ── Ruler ─────────────────────────────────── */
  .ruler-row {
    display: flex;
    height: var(--ruler-h);
    position: sticky;
    top: 0;
    z-index: 2;
    flex-shrink: 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .ruler-corner {
    width: var(--panel-w);
    flex-shrink: 0;
    background: #181b25;
    border-right: 1px solid var(--main-border);
    position: sticky;
    left: 0;
    z-index: 3;
  }

  .ruler-group {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    padding-left: 5px;
    border-right: 1px solid rgba(255, 255, 255, 0.04);
    box-sizing: border-box;
  }

  .bar-num {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.22);
    letter-spacing: 0.03em;
    font-variant-numeric: tabular-nums;
    user-select: none;
    pointer-events: none;
  }

  /* ── Scrollbars ─────────────────────────────── */
  .playlist::-webkit-scrollbar {
    width: 8px;
    height: 8px;
  }

  .playlist::-webkit-scrollbar-track {
    background: #191c25;
  }

  .playlist::-webkit-scrollbar-thumb {
    background: var(--main-border);
    border-radius: 7px;
  }

  .playlist::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.18);
  }

  .playlist::-webkit-scrollbar-corner {
    background: #191c25;
  }
</style>
