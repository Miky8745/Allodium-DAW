<script lang="ts">
  import DragTooltip from './DragTooltip.svelte';
  import { hoverInfoAction } from '$lib/hoverInfo';

  let {
    value        = $bindable(0.8),
    min          = 0,
    max          = 2,
    defaultValue = 0.8,
    height       = 140,
    width        = 28,
    tooltipKey   = '',
    format       = (v: number) => `${Math.round(v * 100)}%`,
    // Drives the Toolbar's info panel while hovered (src/lib/hoverInfo.ts).
    // Empty string (the default) opts out.
    name         = '',
  }: {
    value?:        number;
    min?:          number;
    max?:          number;
    defaultValue?: number;
    height?:       number;
    width?:        number;
    tooltipKey?:   string;
    format?:       (value: number) => string;
    name?:         string;
  } = $props();

  let dragging = $state(false);
  let dragStartY = 0;
  let dragStartVal = 0;

  // Fraction of travel, 0 (min, bottom) to 1 (max, top) — a fader's value
  // is always read off its vertical position, unlike a Dial's angle.
  let frac = $derived(Math.max(0, Math.min(1, (value - min) / (max - min))));

  function clamp(v: number) {
    return Math.max(min, Math.min(max, v));
  }

  function onMousedown(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragging = true;
    dragStartY = e.clientY;
    dragStartVal = value;
  }

  function onMousemove(e: MouseEvent) {
    if (!dragging) return;
    // Absolute delta from drag start (ScrollField's convention, not Dial's
    // movementY accumulation) mapped 1:1 against the fader's own travel
    // height — a physical fader's position should be a driftless function
    // of pixel distance from where the drag started.
    const delta = dragStartY - e.clientY;
    value = clamp(dragStartVal + (delta / height) * (max - min));
  }

  function onMouseup() {
    dragging = false;
  }

  function onDblclick(e: MouseEvent) {
    e.stopPropagation();
    value = defaultValue;
  }

  let wrapEl = $state<HTMLElement | null>(null);
</script>

<svelte:window onmousemove={onMousemove} onmouseup={onMouseup} />

<div
  class="fader-wrap"
  bind:this={wrapEl}
  style="width:{width}px; height:{height}px;"
  onmousedown={onMousedown}
  ondblclick={onDblclick}
  use:hoverInfoAction={{ name, value: format(value) }}
  role="slider"
  aria-valuenow={value}
  aria-valuemin={min}
  aria-valuemax={max}
  tabindex="0"
>
  <div class="fader-track"></div>

  <div class="fader-ticks">
    {#each [0, 0.25, 0.5, 0.75, 1] as t (t)}
      <div class="fader-tick" style="bottom:{t * 100}%"></div>
    {/each}
  </div>

  <div class="fader-cap" style="bottom:calc({frac * 100}% - 7px);">
    <div class="fader-cap-groove"></div>
  </div>

  {#if dragging && tooltipKey && wrapEl}
    <DragTooltip keyLabel={tooltipKey} valueText={format(value)} anchorEl={wrapEl} />
  {/if}
</div>

<style>
  .fader-wrap {
    position: relative;
    cursor: ns-resize;
    user-select: none;
    flex-shrink: 0;
  }

  .fader-track {
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    width: 4px;
    transform: translateX(-50%);
    border-radius: 4px;
    background: #141720;
    box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.8), inset 0 0 0 1px rgba(255, 255, 255, 0.04);
  }

  .fader-ticks {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }

  .fader-tick {
    position: absolute;
    left: 50%;
    width: 9px;
    height: 1px;
    transform: translate(-50%, 50%);
    background: rgba(255, 255, 255, 0.14);
  }

  .fader-cap {
    position: absolute;
    left: 0;
    right: 0;
    height: 14px;
    border-radius: 5px;
    background: linear-gradient(180deg, #484a52, #292b34 45%, #1e212a);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.6), inset 0 1px 0 rgba(255, 255, 255, 0.08);
    display: flex;
    align-items: center;
  }

  .fader-cap-groove {
    width: 100%;
    height: 2px;
    background: rgba(255, 255, 255, 0.3);
    box-shadow: 0 1px 0 rgba(0, 0, 0, 0.5);
  }
</style>
