<script lang="ts">
  // One row in the FX rack's insert-slot list (see MixerFxRack.svelte) — a
  // bypass toggle placeholder, not a real DSP slot (this app has no plugin
  // system). Sized for the wider FX rack panel, not a compact channel strip.
  import type { InsertSlotData } from '$lib/mixerStore.svelte';

  // Bound live to the store's $state object (same convention as
  // MixerChannelStrip/ChannelRow — mutate the passed object directly rather
  // than threading a bindable primitive through).
  let {
    slot,
    index,
  }: {
    slot:  InsertSlotData;
    index: number;
  } = $props();
</script>

<button
  class="slot"
  class:slot--on={slot.enabled}
  onclick={() => slot.enabled = !slot.enabled}
  aria-label="Toggle insert slot {index + 1}"
  aria-pressed={slot.enabled}
  title={slot.enabled ? 'Enabled' : 'Bypassed'}
>
  <span class="slot-index">{index + 1}</span>
  <span class="slot-name">Empty</span>
  <span class="slot-dot" class:slot-dot--on={slot.enabled}></span>
</button>

<style>
  .slot {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    border: none;
    background: transparent;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    cursor: pointer;
    transition: background 0.1s;
  }

  .slot:hover {
    background: rgba(255, 255, 255, 0.04);
  }

  .slot-index {
    width: 14px;
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-size: 10px;
    color: rgba(255, 255, 255, 0.3);
    flex-shrink: 0;
  }

  .slot-name {
    flex: 1;
    text-align: left;
    font-size: 11px;
    color: var(--main-text-muted, #606269);
    font-style: italic;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .slot-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #20232c;
    border: 1px solid rgba(255, 255, 255, 0.15);
    flex-shrink: 0;
    transition: background 0.1s, border-color 0.1s, box-shadow 0.1s;
  }

  .slot-dot--on {
    background: var(--accent, #e6b07f);
    border-color: var(--accent, #e6b07f);
    box-shadow: 0 0 4px rgba(230,176,127, 0.7);
  }
</style>
