<script lang="ts">
  // ChannelRow's right-click > "MIDI out…" sub-view: which output port this
  // channel's piano-roll notes are sent to (only ports enabled in
  // Preferences > MIDI are listed), and on which MIDI channel.
  import type { ChannelData } from '$lib/types';
  import { preferences } from '$lib/preferencesStore.svelte';
  import { preferencesUi } from '$lib/preferencesUi.svelte';
  import ScrollField from './ScrollField.svelte';

  let { channel, onDone }: { channel: ChannelData; onDone: () => void } = $props();

  let ports = $derived(preferences.enabledOutputNames);

  function openPrefs() {
    onDone();
    preferencesUi.show('midi');
  }
</script>

<div class="mo-title">MIDI out</div>
<button class="mo-item" class:mo-item--on={channel.midiOutPort === null} onclick={() => (channel.midiOutPort = null)}>None</button>
{#each ports as port (port)}
  <button class="mo-item" class:mo-item--on={channel.midiOutPort === port} onclick={() => (channel.midiOutPort = port)} title={port}>{port}</button>
{/each}
{#if channel.midiOutPort && !ports.includes(channel.midiOutPort)}
  <button class="mo-item mo-item--on mo-item--missing" title="This port isn't enabled in Preferences">{channel.midiOutPort} (disabled)</button>
{/if}
{#if ports.length === 0}
  <button class="mo-item mo-hint" onclick={openPrefs}>Enable an output in Preferences › MIDI…</button>
{/if}
<div class="mo-row">
  <span>Channel</span>
  <ScrollField bind:value={channel.midiOutChannel} min={1} max={16} step={1} width={32} tooltipKey="MIDI CH" name="MIDI out channel" />
</div>
<button class="mo-item mo-done" onclick={onDone}>Done</button>

<style>
  .mo-title {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--main-text-muted);
    padding: 4px 10px 2px;
  }
  .mo-item {
    background: transparent;
    border: none;
    color: rgba(255,255,255,0.7);
    font-size: 12px;
    text-align: left;
    padding: 5px 10px 5px 22px;
    border-radius: 5px;
    cursor: pointer;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    position: relative;
  }
  .mo-item:hover {
    background: rgba(255,255,255,0.07);
    color: rgba(255,255,255,0.95);
  }
  .mo-item--on::before {
    content: '';
    position: absolute;
    left: 9px;
    top: 50%;
    width: 6px;
    height: 6px;
    margin-top: -3px;
    border-radius: 50%;
    background: var(--accent);
  }
  .mo-item--missing { color: var(--main-text-muted); }
  .mo-hint { color: var(--accent); padding-left: 10px; }
  .mo-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 5px 10px;
    margin-top: 2px;
    border-top: 1px solid rgba(255,255,255,0.08);
    font-size: 12px;
    color: rgba(255,255,255,0.7);
  }
  .mo-done {
    margin-top: 2px;
    text-align: center;
    padding-left: 10px;
    color: var(--accent);
  }
</style>
