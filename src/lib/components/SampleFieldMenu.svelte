<script lang="ts">
  import type { ChannelData } from '$lib/types';
  import { portal } from '$lib/portal';
  import { PLUGINS, assignPlugin, removePlugin, pluginById } from '$lib/plugins/registry';

  // Right-click menu of a channel's sample field (separate from the row's
  // own menu): lists every plugin from the registry. Picking one assigns it
  // to the channel and opens it; the assigned one is checked.
  let { channel, x, y, onClose }: { channel: ChannelData; x: number; y: number; onClose: () => void } = $props();

  function pick(id: string) {
    const channelId = channel.id;
    onClose();
    assignPlugin(channelId, id);
    pluginById(id)?.open(channelId);
  }

  function remove() {
    const channelId = channel.id;
    onClose();
    removePlugin(channelId);
  }
</script>

<div class="sf-root" use:portal>
  <div class="sf-backdrop" onmousedown={onClose} oncontextmenu={(e) => { e.preventDefault(); onClose(); }} role="presentation"></div>
  <div class="sf-menu" style="left:{x}px; top:{y}px;" role="menu">
    <div class="sf-heading">Plugins</div>
    {#each PLUGINS as p (p.id)}
      <button class="sf-item" class:sf-item--on={channel.plugin === p.id} role="menuitemradio" aria-checked={channel.plugin === p.id} onclick={() => pick(p.id)}>
        <span class="sf-check">{channel.plugin === p.id ? '✓' : ''}</span>
        <span class="sf-name">{p.name}</span>
        <span class="sf-kind">{p.kind}</span>
      </button>
    {/each}
    {#if channel.plugin}
      <div class="sf-sep"></div>
      <button class="sf-item sf-item--danger" role="menuitem" onclick={remove}>
        <span class="sf-check"></span>
        <span class="sf-name">Remove plugin</span>
      </button>
    {/if}
  </div>
</div>

<style>
  .sf-root {
    position: fixed;
    inset: 0;
    z-index: 10000;
  }
  .sf-backdrop {
    position: absolute;
    inset: 0;
  }
  .sf-menu {
    position: fixed;
    min-width: 170px;
    padding: 3px;
    background: #272a33;
    border: 1px solid var(--explorer-border, #34373f);
    border-radius: 8px;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.5);
  }
  .sf-heading {
    padding: 4px 10px 3px 28px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--main-text-muted);
  }
  .sf-item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 5px 10px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: rgba(255, 255, 255, 0.7);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    transition: background 0.08s, color 0.08s;
  }
  .sf-item:hover {
    background: rgba(255, 255, 255, 0.07);
    color: rgba(255, 255, 255, 0.95);
  }
  .sf-item--danger:hover {
    background: rgba(220, 50, 50, 0.18);
    color: #f08484;
  }
  .sf-check {
    width: 12px;
    color: var(--accent);
    flex-shrink: 0;
  }
  .sf-name { flex: 1; }
  .sf-kind {
    font-size: 10px;
    color: var(--main-text-muted);
  }
  .sf-sep {
    height: 1px;
    margin: 4px 6px;
    background: rgba(255, 255, 255, 0.08);
  }
</style>
