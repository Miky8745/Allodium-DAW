<script lang="ts">
  import { placementStore } from '$lib/placementStore.svelte';
  import { beginDragCursor, endDragCursor } from '$lib/dragCursor';
  import { patternStore } from '$lib/patternStore.svelte';
  import { channelStore } from '$lib/channelStore.svelte';
  import type { Track } from './PlaylistTrackRow.svelte';

  // Song minimap above the Playlist ruler: one thin lane per track with a
  // rounded block per placement, plus a lighter overlay for the part of
  // the timeline currently in view. Click/drag the overlay (or click
  // anywhere to jump there) to scroll the Playlist horizontally — same
  // idea as the Piano Roll's PianoRollOverview.
  let {
    tracks,
    beatWidth,
    beatCount,
    scrollLeft,
    viewportWidth,
    onScrollTo,
  }: {
    tracks: Track[];
    beatWidth: number;
    beatCount: number;   // whole timeline length (the scroll range)
    scrollLeft: number;  // px, the grid's scrollLeft
    viewportWidth: number; // px of timeline visible (excludes the track panel)
    onScrollTo: (scrollLeftPx: number) => void;
  } = $props();

  const CLIP_COLOR = '#6c91b1'; // matches the Playlist's clip drag ghost
  const PAD_BARS = 2;           // empty headroom shown after the song's end

  let mapEl = $state<HTMLDivElement | null>(null);

  // Same lengths PlaylistTrackRow draws (patternStore.effectiveLengths).
  let blocks = $derived(placementStore.placements.map(p => {
    const pattern = p.kind === 'pattern' ? patternStore.patterns.find(x => x.id === p.patternId) : undefined;
    const len = p.kind === 'pattern'
      ? (pattern ? patternStore.effectiveLengths.get(pattern.id) ?? pattern.lengthBeats : 0)
      : p.lengthBeats;
    return { id: p.id, trackId: p.trackId, start: p.startBeat, len, color: pattern?.color ?? CLIP_COLOR };
  }).filter(b => b.len > 0));

  let viewStart = $derived(scrollLeft / beatWidth);
  let viewBeats = $derived(Math.max(0, viewportWidth / beatWidth));

  // The span the minimap squeezes into its width: the song plus a little
  // headroom, grown to include the viewport if it's scrolled past that, and
  // never more than the whole timeline. Frozen while dragging so the
  // mapping doesn't rescale under the cursor.
  let liveRange = $derived.by(() => {
    const songEnd = blocks.reduce((m, b) => Math.max(m, b.start + b.len), 0);
    const padded = songEnd + PAD_BARS * channelStore.beatsPerBar;
    return Math.max(1, Math.min(beatCount, Math.max(padded, viewStart + viewBeats)));
  });
  let frozenRange = $state<number | null>(null);
  let range = $derived(frozenRange ?? liveRange);

  let laneIndex = $derived(new Map(tracks.map((t, i) => [t.id, i])));
  let laneH = $derived(100 / Math.max(1, tracks.length));

  const pct = (beats: number) => (beats / range) * 100;

  // ── Dragging the view window ──────────────────────────────────────
  let grabOffset = 0; // beats between the window's left edge and the pointer

  function beatAt(clientX: number): number {
    const rect = mapEl!.getBoundingClientRect();
    return ((clientX - rect.left) / Math.max(1, rect.width)) * range;
  }

  function scrollToViewStart(beat: number) {
    const maxStart = Math.max(0, range - viewBeats);
    onScrollTo(Math.max(0, Math.min(maxStart, beat)) * beatWidth);
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0 || !mapEl) return;
    e.preventDefault();
    mapEl.setPointerCapture(e.pointerId);
    frozenRange = liveRange;
    beginDragCursor('ew-resize');
    const beat = beatAt(e.clientX);
    if (beat >= viewStart && beat <= viewStart + viewBeats) {
      grabOffset = beat - viewStart;
    } else {
      // Clicking outside the window centers it there, then keeps dragging.
      grabOffset = viewBeats / 2;
      scrollToViewStart(beat - grabOffset);
    }
  }

  function onPointerMove(e: PointerEvent) {
    if (frozenRange === null) return;
    scrollToViewStart(beatAt(e.clientX) - grabOffset);
  }

  function onPointerUp() {
    if (frozenRange !== null) endDragCursor();
    frozenRange = null;
  }
</script>

<div
  class="ov-map"
  bind:this={mapEl}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={onPointerUp}
  role="scrollbar"
  aria-label="Playlist overview"
  aria-orientation="horizontal"
  aria-controls="playlist-grid"
  aria-valuenow={Math.round(viewStart)}
  tabindex="-1"
>
  {#each blocks as b (b.id)}
    {@const lane = laneIndex.get(b.trackId)}
    {#if lane !== undefined}
      <div
        class="ov-block"
        style="left:{pct(b.start)}%; width:{pct(b.len)}%; top:calc({lane * laneH}% + 0.5px); height:calc({laneH}% - 1px); --bc:{b.color};"
      ></div>
    {/if}
  {/each}
  <div
    class="ov-window"
    class:ov-window--dragging={frozenRange !== null}
    style="left:{pct(viewStart)}%; width:{Math.min(100, pct(viewBeats))}%;"
  ></div>
</div>

<style>
  .ov-map {
    position: relative;
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow: hidden;
    background: #171a23;
  }

  /* Placement blocks: each lane is 1/tracks of the bar; a 1px gap (0.5px
     inset each side, see the inline top/height) keeps lanes apart. */
  .ov-block {
    position: absolute;
    min-width: 2px;
    background-color: var(--bc);
    border-radius: 3px;
    opacity: 0.9;
    pointer-events: none;
  }

  .ov-window {
    position: absolute;
    top: 0;
    bottom: 0;
    box-sizing: border-box;
    min-width: 6px;
    background: rgba(255,255,255,0.1);
    border: 1px solid rgba(255,255,255,0.28);
    border-radius: 5px;
    transition: background 0.1s;
  }

  .ov-map:hover .ov-window { background: rgba(255,255,255,0.14); }

  .ov-window--dragging,
  .ov-map:hover .ov-window--dragging {
    background: rgba(255,255,255,0.18);
  }
</style>
