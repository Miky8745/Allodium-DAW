<script lang="ts">
  import type { PatternData } from '$lib/types';
  import ColorWheel from '../ColorWheel.svelte';
  import { portal } from '$lib/portal';
  import { randomColorfulHex } from '$lib/colorUtils';

  // Right-click menu for a pattern card in the Playlist's patterns panel.
  // Same portal/backdrop/look as ChannelRow's menu; Recolor swaps the menu
  // body for a ColorWheel live-writing pattern.color.
  let {
    pattern,
    x,
    y,
    onRename,
    onDelete,
    onClose,
  }: {
    pattern: PatternData;
    x: number;
    y: number;
    onRename: () => void;
    onDelete: () => void;
    onClose: () => void;
  } = $props();

  let view = $state<'main' | 'recolor'>('main');

  function recolorRandomly() {
    pattern.color = randomColorfulHex();
    onClose();
  }
</script>

<div class="ctx-root" use:portal>
  <div class="ctx-backdrop" onmousedown={onClose} role="presentation"></div>
  <div class="ctx-menu" class:ctx-menu--wide={view === 'recolor'} style="left:{x}px; top:{y}px;" role="menu">
    {#if view === 'main'}
      <button class="ctx-item" role="menuitem" onclick={onRename}>Rename pattern</button>
      <div class="ctx-sep"></div>
      <button class="ctx-item" role="menuitem" onclick={() => (view = 'recolor')}>Recolor</button>
      <button class="ctx-item" role="menuitem" onclick={recolorRandomly}>Recolor randomly</button>
      <div class="ctx-sep"></div>
      <button class="ctx-item ctx-item--danger" role="menuitem" onclick={onDelete}>
        Delete pattern
      </button>
    {:else}
      <ColorWheel bind:value={pattern.color} />
      <button class="ctx-item ctx-done" role="menuitem" onclick={onClose}>Done</button>
    {/if}
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
    min-width: 150px;
    padding: 3px;
    display: flex;
    flex-direction: column;
  }

  .ctx-menu--wide { min-width: 190px; }

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

  .ctx-item--danger:hover {
    background: rgba(220,50,50,0.18);
    color: #f08484;
  }

  .ctx-sep {
    height: 1px;
    margin: 4px 6px;
    background: rgba(255,255,255,0.08);
  }

  .ctx-done {
    margin-top: 2px;
    text-align: center;
    color: var(--accent);
  }
</style>
