<script lang="ts">
  import DragTooltip from './DragTooltip.svelte';
  import KnobContextMenu from './KnobContextMenu.svelte';
  import { hoverInfoAction } from '$lib/hoverInfo';

  // Same drag-to-set contract as Dial.svelte, but the value readout is a
  // full-circle rim (active-colored portion vs. inactive-colored portion)
  // instead of a rotating indicator dot. Currently unused (its former
  // caller, the Mixer's JackPort, was removed when cable-patchbay routing
  // was replaced with per-track routing — see "Mixer" in
  // CLAUDE.md) but kept as a small, generic, documented primitive.
  let {
    value        = $bindable(0),
    defaultValue = 0,
    max          = 1,
    size         = 24,
    sensitivity  = 200,
    activeColor   = 'var(--accent, #e6b07f)',
    inactiveColor = 'rgba(255,255,255,0.18)',
    tooltipKey   = '',
    format       = (v: number) => `${Math.round(v * 100)}%`,
    // Drives the Toolbar's info panel while hovered (src/lib/hoverInfo.ts).
    // Empty string (the default) opts out.
    name         = '',
  }: {
    value?:         number;
    defaultValue?:  number;
    max?:           number;
    size?:          number;
    sensitivity?:   number;
    activeColor?:   string;
    inactiveColor?: string;
    tooltipKey?:    string;
    format?:        (value: number) => string;
    name?:          string;
  } = $props();

  let dragging = $state(false);
  // Drag anchor: the value follows the cursor's vertical distance from where
  // the drag started (position-linear), not the summed movementY deltas —
  // those carry the OS pointer acceleration, so fast drags travelled farther.
  let dragStartY = 0;
  let dragStartValue = 0;

  // SVG canvas: 32×32, radius 12, center (16,16) — same reference frame as Dial.
  const CX = 16, CY = 16, R = 12;

  function pt(deg: number) {
    const rad = (deg - 90) * (Math.PI / 180);
    return { x: CX + R * Math.cos(rad), y: CY + R * Math.sin(rad) };
  }

  // Full background ring — a closed circle can't be a single SVG arc command
  // (start === end), so it's drawn as two semicircle arcs back to back.
  const bgPath = `M ${pt(0).x.toFixed(2)} ${pt(0).y.toFixed(2)} A ${R} ${R} 0 1 1 ${pt(180).x.toFixed(2)} ${pt(180).y.toFixed(2)} A ${R} ${R} 0 1 1 ${pt(0).x.toFixed(2)} ${pt(0).y.toFixed(2)}`;

  // Reactive fill arc, 0deg to (value/max)*360deg
  let fillPath = $derived.by(() => {
    if (value <= 0) return '';
    const sweep = (value / max) * 360;
    if (sweep >= 360) return bgPath;
    const s = pt(0), e = pt(sweep);
    const large = sweep > 180 ? 1 : 0;
    return `M ${s.x.toFixed(2)} ${s.y.toFixed(2)} A ${R} ${R} 0 ${large} 1 ${e.x.toFixed(2)} ${e.y.toFixed(2)}`;
  });

  function onMousedown(e: MouseEvent) {
    // Only the left button drags the value — right-click (button 2) is
    // reserved for a state-cycling gesture in some potential callers, and
    // must not also start a drag gesture here.
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    dragStartY = e.clientY;
    dragStartValue = value;
    dragging = true;
  }

  function onMousemove(e: MouseEvent) {
    if (!dragging) return;
    value = Math.max(0, Math.min(max, dragStartValue + ((dragStartY - e.clientY) / sensitivity) * max));
  }

  function onMouseup() {
    dragging = false;
  }

  function onDblclick(e: MouseEvent) {
    e.stopPropagation();
    value = defaultValue;
  }

  // Right-click reset menu — a slower, more discoverable alternative to the
  // double-click reset above (both put the knob back at defaultValue). Was
  // previously just a reserved no-op (see onMousedown's guard comment);
  // this is what that reservation was for.
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
  class="knob-wrap"
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
  <div class="knob-body"></div>
  <svg class="knob-svg" viewBox="0 0 32 32" width={size} height={size}>
    <path d={bgPath} fill="none" stroke={inactiveColor} stroke-width="2.5" />
    {#if value > 0}
      <path d={fillPath} fill="none" stroke={activeColor} stroke-width="2.5" stroke-linecap="round" />
    {/if}
  </svg>
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
  .knob-wrap {
    position: relative;
    cursor: ns-resize;
    user-select: none;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .knob-body {
    position: absolute;
    inset: 4px;
    border-radius: 50%;
    background: radial-gradient(circle at 40% 35%, #3a3c45, #20232c);
    box-shadow: 0 1px 3px rgba(0,0,0,0.5), inset 0 1px 0 rgba(255,255,255,0.07);
  }

  .knob-svg {
    position: relative;
    display: block;
    overflow: visible;
  }
</style>
