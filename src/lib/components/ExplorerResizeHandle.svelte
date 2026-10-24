<script lang="ts">
  // Draggable vertical grip on the explorer's right edge. Uses pointer
  // capture on the handle itself (no document-level listeners), so the drag
  // keeps working when the cursor leaves the 5px strip — and `width` is the
  // panel's own bindable width, clamped to [min, max].
  let {
    width = $bindable(220),
    min = 140,
    max = 600,
  }: { width?: number; min?: number; max?: number } = $props();

  let startX = 0;
  let startWidth = 0;
  let active = $state(false);

  function onDown(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    startX = e.clientX;
    startWidth = width;
    active = true;
  }

  function onMove(e: PointerEvent) {
    if (!active) return;
    width = Math.max(min, Math.min(max, startWidth + (e.clientX - startX)));
  }

  function onUp() { active = false; }
</script>

<div
  class="handle"
  class:handle--active={active}
  onpointerdown={onDown}
  onpointermove={onMove}
  onpointerup={onUp}
  onpointercancel={onUp}
  role="separator"
  aria-orientation="vertical"
  aria-label="Resize explorer"
></div>

<style>
  .handle {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: 5px;
    cursor: col-resize;
    z-index: 1;
    transition: background 0.1s;
  }
  .handle:hover,
  .handle--active { background: var(--accent); opacity: 0.6; }
</style>
