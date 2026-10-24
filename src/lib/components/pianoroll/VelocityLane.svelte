<script lang="ts">
  import type { ChannelData } from '$lib/types';
  import { stepW, LANE_H, patternWidth, stepToX } from '$lib/pianoroll/pitch';

  let {
    channel,
    selectedNoteIds,
    scrollLeft,
    patternLength,
  }: {
    channel: ChannelData;
    selectedNoteIds: Set<number>;
    scrollLeft: number;
    patternLength: number;
  } = $props();

  let laneEl = $state<HTMLElement | null>(null);
  let dragging = $state(false);

  function velocityFromEvent(e: MouseEvent | PointerEvent): number {
    const rect = laneEl!.getBoundingClientRect();
    const y = e.clientY - rect.top;
    return Math.max(0, Math.min(1, 1 - y / LANE_H));
  }

  function barMousedown(e: MouseEvent, noteId: number) {
    if (e.button !== 0) return;
    dragging = true;
    const note = channel.notes.find(n => n.id === noteId);
    if (note) note.velocity = velocityFromEvent(e);
  }

  function barPointerenter(e: PointerEvent, noteId: number) {
    if (!dragging) return;
    if (e.buttons === 0) { dragging = false; return; }
    const note = channel.notes.find(n => n.id === noteId);
    if (note) note.velocity = velocityFromEvent(e);
  }

  function globalPointerup() { dragging = false; }
</script>

<svelte:window onpointerup={globalPointerup} />

<div class="vel-viewport" bind:this={laneEl} style="height:{LANE_H}px;">
  <div class="vel-content" style="width:{patternWidth(patternLength)}px; transform: translateX(-{scrollLeft}px);">
    {#each Array(patternLength) as _, i}
      <div class="vel-col" class:vel-col--orange={Math.floor(i / 4) % 2 === 1} style="left:{stepToX(i)}px; width:{stepW()}px;"></div>
    {/each}
    {#each channel.notes as note (note.id)}
      <div
        class="vel-bar"
        class:vel-bar--selected={selectedNoteIds.has(note.id)}
        style="left:{stepToX(note.start)}px; width:{note.length * stepW() - 2}px; height:{note.velocity * LANE_H}px;"
        onmousedown={(e) => barMousedown(e, note.id)}
        onpointerenter={(e) => barPointerenter(e, note.id)}
        oncontextmenu={(e) => e.preventDefault()}
        role="slider"
        aria-label="Note velocity"
        aria-valuenow={Math.round(note.velocity * 100)}
        aria-valuemin={0}
        aria-valuemax={100}
        tabindex="-1"
      ></div>
    {/each}
  </div>
</div>

<style>
  .vel-viewport {
    position: relative;
    overflow: hidden;
    background: #1b1e27;
  }

  .vel-content {
    position: relative;
    height: 100%;
  }

  .vel-col {
    position: absolute;
    top: 0;
    bottom: 0;
    background: rgba(255,255,255,0.02);
  }

  .vel-col--orange { background: rgba(230,176,127,0.03); }

  .vel-bar {
    position: absolute;
    bottom: 0;
    background: var(--accent, #e6b07f);
    border-radius: 4px 4px 0 0;
    cursor: ns-resize;
    opacity: 0.8;
  }

  .vel-bar--selected {
    opacity: 1;
    outline: 1px solid rgba(255,255,255,0.6);
    outline-offset: -1px;
  }
</style>
