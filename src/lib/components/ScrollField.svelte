<script lang="ts">
  import DragTooltip from './DragTooltip.svelte';
  import { hoverInfoAction } from '$lib/hoverInfo';

  let {
    value      = $bindable(1),
    min        = 0,
    max        = 999,
    step       = 1,
    decimals   = 0,
    width      = 36,
    label      = '',
    tooltipKey = '',
    // Drives the Toolbar's info panel while hovered (src/lib/hoverInfo.ts).
    // Empty string (the default) opts out.
    name       = '',
  }: {
    value?:      number;
    min?:        number;
    max?:        number;
    step?:       number;
    decimals?:   number;
    width?:      number;
    label?:      string;
    tooltipKey?: string;
    name?:       string;
  } = $props();

  let display = $derived(value.toFixed(decimals));
  let displayWithLabel = $derived(label ? `${display} ${label}` : display);

  let dragStartY = 0;
  let dragStartVal = 0;
  let dragging = $state(false);

  function clamp(v: number) {
    return Math.max(min, Math.min(max, v));
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const dir = e.deltaY < 0 ? 1 : -1;
    value = clamp(value + dir * step);
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
    const delta = dragStartY - e.clientY;
    value = clamp(dragStartVal + Math.round(delta / 4) * step);
  }

  function onMouseup() {
    dragging = false;
  }

  let wrapEl = $state<HTMLElement | null>(null);
</script>

<svelte:window onmousemove={onMousemove} onmouseup={onMouseup} />

<div class="sf-wrap" bind:this={wrapEl} style="width:{width}px;" onwheel={onWheel} onmousedown={onMousedown} use:hoverInfoAction={{ name, value: displayWithLabel }} role="spinbutton" aria-valuenow={value} aria-valuemin={min} aria-valuemax={max} tabindex="0">
  <span class="sf-val">{display}</span>
  {#if label}
    <span class="sf-label">{label}</span>
  {/if}
  {#if dragging && tooltipKey && wrapEl}
    <DragTooltip keyLabel={tooltipKey} valueText={displayWithLabel} anchorEl={wrapEl} />
  {/if}
</div>

<style>
  .sf-wrap {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    background: #171a23;
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 5px;
    padding: 1px 2px;
    cursor: ns-resize;
    user-select: none;
    flex-shrink: 0;
  }

  .sf-val {
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-size: 11px;
    color: var(--main-text);
    line-height: 1.2;
    white-space: nowrap;
  }

  .sf-label {
    font-size: 7px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--main-text-muted);
    line-height: 1;
  }
</style>
