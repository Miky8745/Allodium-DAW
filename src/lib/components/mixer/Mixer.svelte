<script lang="ts">
  import FloatingWindow, { type WorkspaceBounds } from '../FloatingWindow.svelte';
  import MixerChannelStrip from './MixerChannelStrip.svelte';
  import MixerFxRack from './MixerFxRack.svelte';
  import { mixerStore } from '$lib/mixerStore.svelte';

  let { show = $bindable(), workspaceBounds }: { show: boolean; workspaceBounds: WorkspaceBounds } = $props();

  function close() { show = false; }
</script>

<FloatingWindow
  id="mixer"
  bind:show
  {workspaceBounds}
  x={100} y={100} width={900} height={380}
  minWidth={560} minHeight={280}
>
  {#snippet header({ onDragStart, maximized, toggleMaximize })}
    <div class="mixer-header" onmousedown={onDragStart} role="toolbar" aria-label="Mixer controls" tabindex="-1">
      <span class="mixer-title">Mixer</span>
      <div class="hdr-right">
        <button
          class="hdr-btn"
          onclick={(e) => { e.stopPropagation(); toggleMaximize(); }}
          aria-label={maximized ? 'Restore' : 'Maximize'}
          title={maximized ? 'Restore' : 'Maximize'}
        >
          {#if maximized}
            <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
              <rect x="4" y="4" width="8" height="8" rx="1"/>
              <path d="M2 8V3a1 1 0 0 1 1-1h5"/>
            </svg>
          {:else}
            <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
              <rect x="2" y="2" width="10" height="10" rx="1"/>
            </svg>
          {/if}
        </button>
        <button class="close-btn" onclick={close} aria-label="Close">
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <line x1="3" y1="3" x2="11" y2="11"/>
            <line x1="11" y1="3" x2="3" y2="11"/>
          </svg>
        </button>
      </div>
    </div>
  {/snippet}

  <div class="mixer-body">
    <MixerChannelStrip variant="master" data={mixerStore.master} />

    <div class="v-divider"></div>

    <div class="channels-scroll">
      {#each mixerStore.channels as ch (ch.id)}
        <MixerChannelStrip variant="channel" data={ch} onRemove={() => mixerStore.removeChannel(ch.id)} />
      {/each}
      <button class="add-channel-btn" onclick={() => mixerStore.addChannel()} aria-label="Add channel" title="Add channel">+</button>
    </div>

    <div class="v-divider"></div>

    <MixerFxRack />
  </div>
</FloatingWindow>

<style>
  .mixer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px;
    height: 34px;
    background: #1d1f29;
    border-bottom: 1px solid var(--explorer-border, #383c4a);
    flex-shrink: 0;
    user-select: none;
  }

  .mixer-title {
    font-family: 'Playfair Display', Georgia, serif;
    font-size: 14px;
    font-weight: 600;
    color: var(--main-text, #bcbcc0);
    pointer-events: none;
  }

  .hdr-right {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .hdr-btn, .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    color: rgba(255, 255, 255, 0.45);
    cursor: pointer;
    padding: 0;
    transition: color 0.1s, background 0.1s, border-color 0.1s;
  }

  .hdr-btn svg, .close-btn svg { width: 13px; height: 13px; }

  .hdr-btn:hover { background: rgba(255, 255, 255, 0.07); color: rgba(255, 255, 255, 0.75); }

  .close-btn:hover {
    background: rgba(220, 50, 50, 0.18);
    color: #f08484;
    border-color: rgba(220, 50, 50, 0.3);
  }

  .mixer-body {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: stretch;
    background: var(--main-bg, #1e212a);
    overflow: hidden;
  }

  .mixer-body > :global(.strip--master) {
    margin: 12px 0 12px 12px;
  }

  .v-divider {
    width: 1px;
    background: var(--explorer-border, #383c4a);
    flex-shrink: 0;
  }

  .channels-scroll {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: stretch;
    gap: 10px;
    padding: 12px;
    overflow-x: auto;
    overflow-y: hidden;
  }

  .channels-scroll::-webkit-scrollbar { height: 14px; }
  .channels-scroll::-webkit-scrollbar-track { background: transparent; }
  .channels-scroll::-webkit-scrollbar-thumb {
    background: var(--explorer-border, #34373f);
    border-radius: 10px;
    border: 3px solid var(--main-bg, #1e212a);
  }

  .add-channel-btn {
    width: 28px;
    height: 28px;
    align-self: center; /* .channels-scroll stretches its items now that strips fill the full height — this stays a fixed-size button, vertically centered like before */
    border-radius: 7px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    flex-shrink: 0;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
  }

  .add-channel-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--main-text, #bcbcc0);
    border-color: rgba(255, 255, 255, 0.3);
  }
</style>
