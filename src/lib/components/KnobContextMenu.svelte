<script lang="ts">
  // Tiny right-click menu shared by Dial.svelte and Knob.svelte: a single
  // "Reset" item that puts the knob back at its defaultValue. Same
  // portal-onto-body/backdrop/ctx-menu shape as ChannelRow.svelte's own
  // context menu, factored out here since it's now needed identically in
  // more than one place (unlike e.g. the Mixer's rename widget, which is
  // deliberately duplicated because its two contexts look different — this
  // one doesn't).
  import { portal } from '$lib/portal';

  let { x, y, onReset, onClose }: { x: number; y: number; onReset: () => void; onClose: () => void } = $props();

  function reset() {
    onReset();
    onClose();
  }
</script>

<div class="ctx-root" use:portal>
  <div class="ctx-backdrop" onmousedown={onClose} oncontextmenu={(e) => e.preventDefault()} role="presentation"></div>
  <div class="ctx-menu" style="left:{x}px; top:{y}px;" role="menu">
    <button class="ctx-item" role="menuitem" onclick={reset}>Reset</button>
  </div>
</div>

<style>
  .ctx-root {
    position: fixed;
    inset: 0;
    z-index: 10000;
  }

  .ctx-backdrop {
    position: absolute;
    inset: 0;
  }

  .ctx-menu {
    position: fixed;
    background: #272a33;
    border: 1px solid var(--explorer-border, #34373f);
    border-radius: 8px;
    box-shadow: 0 6px 18px rgba(0,0,0,0.5);
    min-width: 110px;
    padding: 3px;
    display: flex;
    flex-direction: column;
  }

  .ctx-item {
    background: transparent;
    border: none;
    color: rgba(255,255,255,0.7);
    font-size: 12px;
    text-align: left;
    padding: 5px 10px;
    border-radius: 5px;
    cursor: pointer;
    transition: background 0.08s, color 0.08s;
  }

  .ctx-item:hover {
    background: rgba(255,255,255,0.07);
    color: rgba(255,255,255,0.95);
  }
</style>
