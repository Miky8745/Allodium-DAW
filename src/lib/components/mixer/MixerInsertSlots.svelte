<script lang="ts">
  // The scrollable stack of insert-slot rows inside the FX rack
  // (MixerFxRack.svelte) — fills whatever vertical space its parent gives
  // it (the FX rack's own panel height), rather than a small fixed cap.
  import type { InsertSlotData } from '$lib/mixerStore.svelte';
  import MixerInsertSlot from './MixerInsertSlot.svelte';

  let { slots }: { slots: InsertSlotData[] } = $props();
</script>

<div class="insert-slots">
  {#each slots as slot, i (slot.id)}
    <MixerInsertSlot {slot} index={i} />
  {/each}
</div>

<style>
  .insert-slots {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .insert-slots::-webkit-scrollbar {
    width: 6px;
  }

  .insert-slots::-webkit-scrollbar-track {
    background: transparent;
  }

  .insert-slots::-webkit-scrollbar-thumb {
    background: var(--explorer-border, #34373f);
    border-radius: 5px;
  }
</style>
