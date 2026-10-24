<script lang="ts">
  import type { ChannelData } from '$lib/types';
  import { beginDragCursor, endDragCursor } from '$lib/dragCursor';
  import { patternWidth } from '$lib/pianoroll/pitch';
  import MiniNoteRoll from './MiniNoteRoll.svelte';

  let {
    channel,
    patternLength,
    activeStep = -1,
    scrollLeft,
    viewportWidth,
    onSeek,
  }: {
    channel: ChannelData;
    patternLength: number;
    activeStep?: number;
    scrollLeft: number;
    viewportWidth: number;
    onSeek: (scrollLeft: number) => void;
  } = $props();

  let wrapEl = $state<HTMLElement | null>(null);
  let dragging = false;

  let patternWidthPx = $derived(patternWidth(patternLength));
  let overlayLeftPct = $derived(patternWidthPx > 0 ? (scrollLeft / patternWidthPx) * 100 : 0);
  let overlayWidthPct = $derived(patternWidthPx > 0 ? Math.min(100, (viewportWidth / patternWidthPx) * 100) : 100);

  function seekTo(clientX: number) {
    if (!wrapEl) return;
    const rect = wrapEl.getBoundingClientRect();
    const frac = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    const target = frac * patternWidthPx - viewportWidth / 2;
    const max = Math.max(0, patternWidthPx - viewportWidth);
    onSeek(Math.max(0, Math.min(max, target)));
  }

  function handlePointerdown(e: PointerEvent) {
    if (e.button !== 0) return;
    dragging = true;
    beginDragCursor('ew-resize');
    seekTo(e.clientX);
  }

  function handlePointermove(e: PointerEvent) {
    if (!dragging) return;
    if (e.buttons === 0) { dragging = false; endDragCursor(); return; }
    seekTo(e.clientX);
  }

  function handlePointerup() {
    if (dragging) endDragCursor();
    dragging = false;
  }
</script>

<svelte:window onpointermove={handlePointermove} onpointerup={handlePointerup} />

<div class="ov-wrap" bind:this={wrapEl} onpointerdown={handlePointerdown} role="slider" aria-label="Pattern overview" aria-valuenow={overlayLeftPct} tabindex="0">
  <MiniNoteRoll notes={channel.notes} {patternLength} {activeStep} fit="stretch" rowHeight={36} />
  <div class="ov-viewport" style="left:{overlayLeftPct}%; width:{overlayWidthPct}%;"></div>
</div>

<style>
  .ov-wrap {
    position: relative;
    width: 100%;
    height: 100%;
  }

  .ov-viewport {
    position: absolute;
    top: 0;
    bottom: 0;
    background: rgba(255,255,255,0.12);
    border-left: 1px solid rgba(255,255,255,0.25);
    border-right: 1px solid rgba(255,255,255,0.25);
    pointer-events: none;
  }
</style>
