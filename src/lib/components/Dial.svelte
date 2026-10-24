<script lang="ts">
  import DragTooltip from './DragTooltip.svelte';
  import KnobContextMenu from './KnobContextMenu.svelte';
  import { hoverInfoAction } from '$lib/hoverInfo';

  let {
    value        = $bindable(0),
    defaultValue = 0,
    max          = 1,
    showArc      = false,
    size         = 28,
    sensitivity  = 200,
    // 300° sweep starting at 210° (lower-left) by default — the classic
    // full-travel rotary-pot look, gap centered at the bottom. A dial that
    // wants its max to land on the right instead (e.g. Volume, see
    // ChannelRow) overrides these to sweep left→top→right instead.
    sweepStart   = 210,
    sweepDegrees = 300,
    tooltipKey   = '',
    format       = (v: number) => `${Math.round(v * 100)}%`,
    // Drives the Toolbar's info panel while hovered (src/lib/hoverInfo.ts)
    // — e.g. "Volume dial" / "80%". Empty string (the default) opts out.
    name         = '',
  }: {
    value?:        number;
    defaultValue?: number;
    max?:          number;
    showArc?:      boolean;
    size?:         number;
    sensitivity?:  number;
    sweepStart?:   number;
    sweepDegrees?: number;
    tooltipKey?:   string;
    format?:       (value: number) => string;
    name?:         string;
  } = $props();

  let dragging = $state(false);

  let angle = $derived(sweepStart + (value / max) * sweepDegrees);

  // SVG canvas: 32×32, radius 12, center (16,16)
  const CX = 16, CY = 16, R = 12;

  function pt(deg: number) {
    const rad = (deg - 90) * (Math.PI / 180);
    return { x: CX + R * Math.cos(rad), y: CY + R * Math.sin(rad) };
  }

  // Background arc path spanning the full sweep
  let bgPath = $derived.by(() => {
    const s = pt(sweepStart), e = pt(sweepStart + sweepDegrees);
    const large = sweepDegrees > 180 ? 1 : 0;
    return `M ${s.x.toFixed(2)} ${s.y.toFixed(2)} A ${R} ${R} 0 ${large} 1 ${e.x.toFixed(2)} ${e.y.toFixed(2)}`;
  });

  // Reactive fill arc path
  let fillPath = $derived.by(() => {
    if (value <= 0) return '';
    const s = pt(sweepStart);
    const sweep = (value / max) * sweepDegrees;
    const e = pt(sweepStart + sweep);
    const large = sweep > 180 ? 1 : 0;
    return `M ${s.x.toFixed(2)} ${s.y.toFixed(2)} A ${R} ${R} 0 ${large} 1 ${e.x.toFixed(2)} ${e.y.toFixed(2)}`;
  });

  // Indicator dot position on the arc
  let tipX = $derived(CX + (R - 2.5) * Math.cos((angle - 90) * Math.PI / 180));
  let tipY = $derived(CY + (R - 2.5) * Math.sin((angle - 90) * Math.PI / 180));

  function onMousedown(e: MouseEvent) {
    // Only the left button drags the value — right-click opens the reset
    // context menu instead (see onContextmenu below) and must not also
    // start a drag.
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    dragging = true;
  }

  function onMousemove(e: MouseEvent) {
    if (!dragging) return;
    // sensitivity is "pixels to sweep the full range", so the delta is
    // scaled by max — otherwise a wider range (e.g. Volume's 0-2) would
    // whip past in half the drag distance of a 0-1 dial.
    value = Math.max(0, Math.min(max, value - (e.movementY / sensitivity) * max));
  }

  function onMouseup() {
    dragging = false;
  }

  function onDblclick(e: MouseEvent) {
    e.stopPropagation();
    value = defaultValue;
  }

  // Right-click reset menu — a slower, more discoverable alternative to the
  // double-click reset above (both put the knob back at defaultValue).
  let contextMenu = $state<{ x: number; y: number } | null>(null);

  function onContextmenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    contextMenu = { x: e.clientX, y: e.clientY };
  }

  let wrapEl = $state<HTMLElement | null>(null);
</script>

<svelte:window onmousemove={onMousemove} onmouseup={onMouseup} />

<div
  class="dial-wrap"
  bind:this={wrapEl}
  style="width:{size}px; height:{size}px;"
  onmousedown={onMousedown}
  ondblclick={onDblclick}
  oncontextmenu={onContextmenu}
  use:hoverInfoAction={{ name, value: format(value) }}
  role="slider"
  aria-valuenow={value}
  aria-valuemin={0}
  aria-valuemax={max}
  tabindex="0"
>
  {#if showArc}
    <svg class="dial-svg" viewBox="0 0 32 32" width={size} height={size}>
      <path d={bgPath} fill="none" stroke="rgba(255,255,255,0.1)" stroke-width="2.5" stroke-linecap="round" />
      {#if value > 0}
        <path d={fillPath} fill="none" stroke="var(--accent)" stroke-width="2.5" stroke-linecap="round" />
      {/if}
      <circle cx={tipX} cy={tipY} r="2" fill="var(--accent)" />
    </svg>
  {:else}
    <div class="knob" style="width:{size}px; height:{size}px; transform: rotate({angle}deg);">
      <div class="knob-dot"></div>
    </div>
  {/if}
  {#if dragging && tooltipKey && wrapEl}
    <DragTooltip keyLabel={tooltipKey} valueText={format(value)} anchorEl={wrapEl} />
  {/if}
</div>

{#if contextMenu}
  <KnobContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    onReset={() => { value = defaultValue; }}
    onClose={() => { contextMenu = null; }}
  />
{/if}

<style>
  .dial-wrap {
    position: relative;
    cursor: ns-resize;
    user-select: none;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .dial-svg {
    display: block;
    overflow: visible;
  }

  .knob {
    border-radius: 50%;
    background: radial-gradient(circle at 40% 35%, #3a3c45, #20232c);
    border: 1px solid rgba(255,255,255,0.12);
    position: relative;
    box-shadow: 0 1px 3px rgba(0,0,0,0.5), inset 0 1px 0 rgba(255,255,255,0.07);
    flex-shrink: 0;
  }

  .knob-dot {
    position: absolute;
    top: 3px;
    left: 50%;
    transform: translateX(-50%);
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: rgba(255,255,255,0.75);
  }
</style>
