<script lang="ts" module>
  export interface Track {
    id: number;
    name: string;
    color: string;
  }
</script>

<script lang="ts">
  import { deleteOut } from '$lib/deleteFx';
  // One Playlist timeline row: beat-cell shading, placed pattern/clip blocks,
  // and the file-drop target that turns a dropped sample into a clip
  // placement. Extracted out of Playlist.svelte (mirrors the
  // ChannelRow.svelte extraction precedent) — the pointer gestures
  // themselves live in playlistEditor.svelte.ts; this component owns only
  // its own row rendering and drop wiring.
  import { placementStore } from '$lib/placementStore.svelte';
  import { patternStore } from '$lib/patternStore.svelte';
  import { channelStore } from '$lib/channelStore.svelte';
  import type { Placement, FileDropDetail, PatternData } from '$lib/types';
  import type { EditTool } from '$lib/editTools';
  import PlaylistPatternPreview from './PlaylistPatternPreview.svelte';
  import PlaylistClipWaveform from './PlaylistClipWaveform.svelte';

  let {
    track,
    beatCount,
    pixelsPerBeat,
    trackPanelW,
    gridEl,
    tool,
    movingIds,
    selectedIds,
    onLanePointerDown,
    onPlacementPointerDown,
    onDeletePointerDown,
    onClipDrop,
  }: {
    track: Track;
    beatCount: number;
    pixelsPerBeat: number;
    trackPanelW: number;
    gridEl: HTMLDivElement | undefined;
    tool: EditTool; // only picks the cursor here
    movingIds: ReadonlySet<number>; // blocks currently being live-moved (raised)
    selectedIds: ReadonlySet<number>;
    onLanePointerDown: (e: PointerEvent, trackId: number) => void; // left button on empty lane space
    onPlacementPointerDown: (e: PointerEvent, placement: Placement) => void;
    onDeletePointerDown: (e: PointerEvent) => void; // right button: delete / drag-delete blocks
    onClipDrop: (trackId: number, startBeat: number, samplePath: string) => void;
  } = $props();

  const SHADE_A = '#20232c';
  const SHADE_B = '#252831';

  function cellBg(beatIndex: number): string {
    const bar = Math.floor(beatIndex / channelStore.beatsPerBar);
    const group = Math.floor(bar / 4);
    return group % 2 === 0 ? SHADE_B : SHADE_A;
  }

  function patternOf(id: number) {
    return patternStore.patterns.find(p => p.id === id);
  }

  // patternStore.effectiveLengths is the same memoized map songTimeline.
  // svelte.ts's audio scheduling reads — so the block drawn here always
  // matches what Song mode will actually schedule, and the underlying
  // content scan only ever runs once per change, not once per reader.
  function effectiveLengthBeats(pattern: PatternData): number {
    return patternStore.effectiveLengths.get(pattern.id) ?? pattern.lengthBeats;
  }

  function sampleBasename(path: string): string {
    return path.split('/').pop() ?? path;
  }

  let rowEl = $state<HTMLDivElement | undefined>();
  let dropHover = $state(false);

  $effect(() => {
    if (!rowEl) return;
    const el = rowEl;
    function onEnter() { dropHover = true; }
    function onLeave() { dropHover = false; }
    function onDrop(e: Event) {
      dropHover = false;
      const detail = (e as CustomEvent<FileDropDetail>).detail;
      if (detail.kind !== 'file' || !gridEl) return; // folder drops on the Playlist are a no-op
      const rect = gridEl.getBoundingClientRect();
      const rawBeat = (detail.x - rect.left - trackPanelW) / pixelsPerBeat;
      onClipDrop(track.id, Math.max(0, Math.round(rawBeat)), detail.path);
    }
    el.addEventListener('filedragenter', onEnter);
    el.addEventListener('filedragleave', onLeave);
    el.addEventListener('filedrop', onDrop);
    return () => {
      el.removeEventListener('filedragenter', onEnter);
      el.removeEventListener('filedragleave', onLeave);
      el.removeEventListener('filedrop', onDrop);
    };
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="track-row"
  class:drop-hover={dropHover}
  class:track-row--delete={tool === 'delete'}
  class:track-row--marquee={tool === 'marquee'}
  class:has-selection={selectedIds.size > 0}
  data-track-id={track.id}
  data-sample-drop
  bind:this={rowEl}
  onpointerdown={(e) => {
    if (e.button === 2) onDeletePointerDown(e); // on a block or anywhere in the lane
    else if (!(e.target as HTMLElement).closest('.track-panel')) onLanePointerDown(e, track.id);
  }}
>
  <div class="track-panel" style="--tc: {track.color}">
    <span class="color-strip"></span>
    <span class="track-name-text">{track.name}</span>
  </div>
  {#each Array.from({ length: beatCount }) as _, bi}
    <div class="beat-cell" class:group-start={bi % channelStore.beatsPerBar === 0} style="background: {cellBg(bi)};"></div>
  {/each}
  <!-- out:deleteOut is |global because the blocks sit inside {#if}s within
       the each item (a local transition wouldn't play when the item goes);
       unmarked removals (moves between tracks, undo) still vanish instantly. -->
  {#each placementStore.placements.filter(p => p.trackId === track.id) as placement (placement.id)}
    {#if placement.kind === 'pattern'}
      {@const pattern = patternOf(placement.patternId)}
      {#if pattern}
        {@const lenBeats = effectiveLengthBeats(pattern)}
        <div
          class="placed-pattern"
          class:moving={movingIds.has(placement.id)}
          class:selected={selectedIds.has(placement.id)}
          data-del-key="placement:{placement.id}"
          out:deleteOut|global
          style="left: {trackPanelW + placement.startBeat * pixelsPerBeat}px; width: {lenBeats * pixelsPerBeat}px; --pc: {pattern.color}"
          onpointerdown={(e) => onPlacementPointerDown(e, placement)}
          data-placement-id={placement.id}
          title="{pattern.name} ({lenBeats.toFixed(2)} beats)"
          role="button"
          tabindex="-1"
        >
          <div class="preview-fill">
            <PlaylistPatternPreview patternId={pattern.id} lengthBeats={lenBeats} />
          </div>
          <span class="placed-pattern-name">{pattern.name}</span>
        </div>
      {/if}
    {:else}
      <div
        class="placed-clip"
        class:moving={movingIds.has(placement.id)}
        class:selected={selectedIds.has(placement.id)}
        data-del-key="placement:{placement.id}"
        out:deleteOut|global
        style="left: {trackPanelW + placement.startBeat * pixelsPerBeat}px; width: {placement.lengthBeats * pixelsPerBeat}px"
        onpointerdown={(e) => onPlacementPointerDown(e, placement)}
        data-placement-id={placement.id}
        title="{sampleBasename(placement.samplePath)} ({placement.lengthBeats.toFixed(2)} beats)"
        role="button"
        tabindex="-1"
      >
        <PlaylistClipWaveform samplePath={placement.samplePath} />
        <span class="placed-clip-name">{sampleBasename(placement.samplePath)}</span>
      </div>
    {/if}
  {/each}
</div>

<style>
  .track-row {
    display: flex;
    position: relative;
    height: var(--track-h);
    flex-shrink: 0;
    border-bottom: 1px solid rgba(0, 0, 0, 0.35);
    cursor: cell; /* Draw tool: lanes draw the active pattern */
  }

  /* Delete / Drag-select tools: their cursor over lanes and blocks alike. */
  .track-row--delete,
  .track-row--delete .beat-cell,
  .track-row--delete .placed-pattern,
  .track-row--delete .placed-clip { cursor: not-allowed; }

  .track-row--marquee,
  .track-row--marquee .beat-cell,
  .track-row--marquee .placed-pattern,
  .track-row--marquee .placed-clip { cursor: crosshair; }

  .track-row.drop-hover {
    background: rgba(230,176,127, 0.06);
  }

  .track-panel {
    width: var(--panel-w);
    flex-shrink: 0;
    cursor: default;
    display: flex;
    align-items: center;
    gap: 8px;
    background: #1d1f29;
    border-right: 1px solid var(--main-border);
    position: sticky;
    left: 0;
    z-index: 1;
    padding: 0 10px;
    overflow: hidden;
  }

  .color-strip {
    width: 3px;
    height: 20px;
    border-radius: 4px;
    background: var(--tc);
    flex-shrink: 0;
  }

  .track-name-text {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    user-select: none;
  }

  .beat-cell {
    width: var(--beat-w);
    flex-shrink: 0;
    border-right: 1px solid rgba(0, 0, 0, 0.25);
    box-sizing: border-box;
    transition: background 0.08s;
  }

  .beat-cell.group-start {
    border-left: 1px solid rgba(255, 255, 255, 0.06);
  }

  .beat-cell:hover {
    background: rgba(255, 255, 255, 0.07) !important;
  }

  .placed-pattern,
  .placed-clip {
    cursor: default; /* blocks are dragged (move cursor while dragging), not drawn on */
    position: absolute;
    top: 8px;
    bottom: 8px;
    z-index: 0;
    overflow: hidden;
    box-sizing: border-box;
    border-radius: 5px;
  }

  .placed-pattern.moving,
  .placed-clip.moving {
    z-index: 1;
    filter: brightness(1.15);
    box-shadow: 0 3px 10px rgba(0, 0, 0, 0.45);
  }

  /* Same idea as selected Piano Roll notes: white rim + glow, every other
     block (on any track — has-selection comes from the shared set) dimmed
     while a selection exists. */
  .placed-pattern.selected,
  .placed-clip.selected {
    z-index: 1;
    box-shadow: 0 0 0 1.5px rgba(255, 255, 255, 0.85), 0 0 8px rgba(255, 255, 255, 0.25);
  }

  .has-selection .placed-pattern:not(.selected),
  .has-selection .placed-clip:not(.selected) {
    opacity: 0.5;
  }

  .placed-pattern {
    background: color-mix(in srgb, var(--pc) 30%, #20232c);
    border: 1px solid var(--pc);
  }

  .placed-clip {
    background: #292f34;
    border: 1px solid #6c91b1;
  }

  /* Fills the whole block; forces MiniNoteRoll's fit="stretch" root (whose
     own inline height is a fixed px value, not 100%) to actually fill it —
     scoped to previews nested inside this component's own blocks only, via
     Svelte's ancestor-scoped :global(), so this can't leak into unrelated
     MiniNoteRoll usages elsewhere (e.g. PianoRollOverview's own stretch
     mode). */
  .preview-fill {
    position: absolute;
    inset: 0;
  }
  .preview-fill :global(.mini-roll--stretch) {
    height: 100% !important;
  }

  .placed-pattern-name,
  .placed-clip-name {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 1;
    max-width: 100%;
    font-size: 10px;
    color: #d6d7d9;
    background: rgba(0, 0, 0, 0.55);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    user-select: none;
    pointer-events: none;
    padding: 1px 5px;
    border-radius: 0 0 5px 0;
  }
</style>
