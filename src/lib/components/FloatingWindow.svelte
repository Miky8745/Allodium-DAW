<script lang="ts">
  import type { Snippet } from 'svelte';
  import { beginDragCursor, endDragCursor } from '$lib/dragCursor';
  import { windowFocus } from '$lib/windowFocusStore.svelte';

  export interface WorkspaceBounds {
    x: number;
    y: number;
    width: number;
    height: number;
  }

  interface HeaderProps {
    onDragStart: (e: MouseEvent) => void;
    maximized: boolean;
    toggleMaximize: () => void;
  }

  let {
    id,
    show = $bindable(true),
    x = $bindable(120),
    y = $bindable(80),
    width = $bindable(720),
    height = $bindable(420),
    maximized = $bindable(false),
    minWidth = 320,
    minHeight = 180,
    workspaceBounds,
    header,
    children,
  }: {
    id: string;
    show?: boolean;
    x?: number;
    y?: number;
    width?: number;
    height?: number;
    maximized?: boolean;
    minWidth?: number;
    minHeight?: number;
    workspaceBounds: WorkspaceBounds;
    header: Snippet<[HeaderProps]>;
    children: Snippet;
  } = $props();

  // ── Window drag ──────────────────────────────────────────────────────
  let dragging = false;
  let dragOffsetX = 0;
  let dragOffsetY = 0;

  function onDragStart(e: MouseEvent) {
    if (maximized) return;
    const t = e.target as HTMLElement;
    if (t.closest('button, [role="slider"], [role="spinbutton"], .resize-handle')) return;
    dragging = true;
    beginDragCursor('move');
    dragOffsetX = e.clientX - x;
    dragOffsetY = e.clientY - y;
    e.preventDefault();
  }

  // ── Resize (8 handles: 4 edges + 4 corners) ───────────────────────────
  type Edge = 'n' | 's' | 'e' | 'w' | 'ne' | 'nw' | 'se' | 'sw';
  let resizing: Edge | null = null;
  let resizeStart = { x: 0, y: 0, w: 0, h: 0, left: 0, top: 0 };

  function onResizeStart(edge: Edge, e: MouseEvent) {
    if (maximized) return;
    e.stopPropagation();
    e.preventDefault();
    resizing = edge;
    beginDragCursor(`${edge}-resize`);
    resizeStart = { x: e.clientX, y: e.clientY, w: width, h: height, left: x, top: y };
  }

  function onWindowMousemove(e: MouseEvent) {
    if (dragging) {
      x = e.clientX - dragOffsetX;
      y = e.clientY - dragOffsetY;
    }
    if (resizing) {
      const dx = e.clientX - resizeStart.x;
      const dy = e.clientY - resizeStart.y;
      if (resizing.includes('e')) {
        width = Math.max(minWidth, resizeStart.w + dx);
      }
      if (resizing.includes('s')) {
        height = Math.max(minHeight, resizeStart.h + dy);
      }
      if (resizing.includes('w')) {
        const newW = Math.max(minWidth, resizeStart.w - dx);
        x = resizeStart.left + (resizeStart.w - newW);
        width = newW;
      }
      if (resizing.includes('n')) {
        const newH = Math.max(minHeight, resizeStart.h - dy);
        y = resizeStart.top + (resizeStart.h - newH);
        height = newH;
      }
    }
  }

  function onWindowMouseup() {
    if (dragging || resizing) endDragCursor();
    dragging = false;
    resizing = null;
  }

  function toggleMaximize() {
    maximized = !maximized;
  }

  // Maximizing pins the window to the workspace zone (where the Playlist
  // normally lives) instead of the whole viewport — never covers the
  // toolbar or the file explorer. The pre-maximize x/y/width/height are
  // left untouched so restoring snaps back to exactly where it was.
  let rectStyle = $derived(
    (maximized
      ? `left:${workspaceBounds.x}px; top:${workspaceBounds.y}px; width:${workspaceBounds.width}px; height:${workspaceBounds.height}px;`
      : `left:${x}px; top:${y}px; width:${width}px; height:${height}px;`)
    + ` z-index:${windowFocus.zIndexOf(id)};`
  );

  function onWindowPointerdown() {
    windowFocus.focus(id);
  }
</script>

<svelte:window onmousemove={onWindowMousemove} onmouseup={onWindowMouseup} />

{#if show}
  <div
    class="fw"
    class:fw--maximized={maximized}
    role="dialog"
    tabindex="-1"
    style={rectStyle}
    onpointerdown={onWindowPointerdown}
  >
    {@render header({ onDragStart, maximized, toggleMaximize })}
    <div class="fw-body">
      {@render children()}
    </div>
    {#if !maximized}
      <div class="resize-handle resize-e"  onmousedown={(e) => onResizeStart('e', e)}  role="presentation"></div>
      <div class="resize-handle resize-w"  onmousedown={(e) => onResizeStart('w', e)}  role="presentation"></div>
      <div class="resize-handle resize-n"  onmousedown={(e) => onResizeStart('n', e)}  role="presentation"></div>
      <div class="resize-handle resize-s"  onmousedown={(e) => onResizeStart('s', e)}  role="presentation"></div>
      <div class="resize-handle resize-ne" onmousedown={(e) => onResizeStart('ne', e)} role="presentation"></div>
      <div class="resize-handle resize-nw" onmousedown={(e) => onResizeStart('nw', e)} role="presentation"></div>
      <div class="resize-handle resize-se" onmousedown={(e) => onResizeStart('se', e)} role="presentation"></div>
      <div class="resize-handle resize-sw" onmousedown={(e) => onResizeStart('sw', e)} role="presentation"></div>
    {/if}
  </div>
{/if}

<style>
  .fw {
    position: fixed;
    background: var(--explorer-bg, #22252e);
    border: 1px solid var(--explorer-border, #383c4a);
    border-radius: 9px;
    display: flex;
    flex-direction: column;
    box-shadow: 0 16px 48px rgba(8,10,20,0.4);
    /* z-index is set dynamically via inline style from windowFocusStore */
    overflow: hidden;
  }

  .fw--maximized {
    border-radius: 0;
  }

  .fw-body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .resize-handle {
    position: absolute;
    z-index: 5;
  }

  .resize-e { top: 6px; bottom: 6px; right: -2px; width: 6px; cursor: e-resize; }
  .resize-w { top: 6px; bottom: 6px; left: -2px;  width: 6px; cursor: w-resize; }
  .resize-n { left: 6px; right: 6px; top: -2px;    height: 6px; cursor: n-resize; }
  .resize-s { left: 6px; right: 6px; bottom: -2px; height: 6px; cursor: s-resize; }

  .resize-ne { top: -2px; right: -2px; width: 12px; height: 12px; cursor: ne-resize; }
  .resize-nw { top: -2px; left: -2px;  width: 12px; height: 12px; cursor: nw-resize; }
  .resize-se { bottom: -2px; right: -2px; width: 12px; height: 12px; cursor: se-resize; }
  .resize-sw { bottom: -2px; left: -2px;  width: 12px; height: 12px; cursor: sw-resize; }
</style>
