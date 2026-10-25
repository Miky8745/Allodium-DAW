<script lang="ts">
  import FloatingWindow, { type WorkspaceBounds } from '../FloatingWindow.svelte';
  import PresetBrowser from './PresetBrowser.svelte';
  import InstrumentPanel from './InstrumentPanel.svelte';
  import { channelStore } from '$lib/channelStore.svelte';
  import { formatSourceLabel } from '$lib/sampleName';
  import { applyPreset } from '$lib/instrument/applyPreset';
  import { instrumentWindow, INSTRUMENT_WINDOW_ID } from '$lib/instrument/instrumentWindow.svelte';
  import { audioEngine } from '$lib/audioEngine';

  // Bloom — the preset-based instrument window. Edits whichever channel is
  // selected (same targeting as the Piano Roll).
  let { workspaceBounds }: { workspaceBounds: WorkspaceBounds } = $props();

  let channel = $derived(channelStore.selectedChannel);

  // Shown with nothing selected (e.g. the channel was deleted): target the
  // first channel.
  $effect(() => {
    if (instrumentWindow.show && !channelStore.selectedChannel && channelStore.channels.length > 0) {
      channelStore.selectedChannelId = channelStore.channels[0].id;
    }
  });

  async function pick(path: string) {
    if (!channel) return;
    const id = channel.id;
    if (await applyPreset(id, path)) {
      const ch = channelStore.channels.find(c => c.id === id);
      if (ch) audioEngine.previewNote(ch, 60, undefined, 0.9);
    }
  }
</script>

<FloatingWindow
  id={INSTRUMENT_WINDOW_ID}
  bind:show={instrumentWindow.show}
  {workspaceBounds}
  x={160} y={90} width={760} height={430}
  minWidth={600} minHeight={360}
>
  {#snippet header({ onDragStart, maximized, toggleMaximize })}
    <div class="in-header" onmousedown={onDragStart} role="toolbar" aria-label="Instrument controls" tabindex="-1">
      <span class="in-title">Bloom <span class="in-sub">{channel ? `— ${channel.name ?? formatSourceLabel(channel)}` : ''}</span></span>
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
        <button class="close-btn" onclick={() => (instrumentWindow.show = false)} aria-label="Close">
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <line x1="3" y1="3" x2="11" y2="11"/>
            <line x1="11" y1="3" x2="3" y2="11"/>
          </svg>
        </button>
      </div>
    </div>
  {/snippet}

  <div class="in-body">
    <PresetBrowser currentPath={channel?.instrument.enabled ? channel.instrument.presetPath : ''} onpick={pick} />
    {#if channel}
      <InstrumentPanel {channel} />
    {:else}
      <p class="in-empty">Add a channel in the Channel Rack to use an instrument.</p>
    {/if}
  </div>
</FloatingWindow>

<style>
  .in-header {
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
  .in-title {
    font-family: 'Playfair Display', Georgia, serif;
    font-size: 14px;
    font-weight: 600;
    color: var(--main-text, #bcbcc0);
    pointer-events: none;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .in-sub {
    font-family: 'DM Sans', 'Segoe UI', system-ui, sans-serif;
    font-weight: 400;
    font-size: 12px;
    color: var(--main-text-muted);
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
  }
  .hdr-btn svg, .close-btn svg { width: 13px; height: 13px; }
  .hdr-btn:hover { background: rgba(255, 255, 255, 0.07); color: rgba(255, 255, 255, 0.75); }
  .close-btn:hover {
    background: rgba(220, 50, 50, 0.18);
    color: #f08484;
    border-color: rgba(220, 50, 50, 0.3);
  }
  .in-body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 190px 1fr;
    background: var(--main-bg, #1e212a);
    overflow: hidden;
  }
  .in-empty {
    margin: 16px;
    font-size: 12px;
    color: var(--main-text-muted);
  }
</style>
