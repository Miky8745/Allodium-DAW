<script lang="ts">
  // Mute (circle) + solo (square) pair for a mixer channel strip. Shape
  // tells them apart rather than color, since the app only has one accent
  // color token (--accent) — both light up the same way ChannelRow's
  // .mute-dot does. Mutates data.muted/data.soloed directly, same
  // direct-mutation convention as channel.muted in ChannelRow.
  import type { MixerChannelData } from '$lib/mixerStore.svelte';

  let { data }: { data: MixerChannelData } = $props();
</script>

<div class="mute-solo">
  <button
    class="ms-btn ms-btn--mute"
    class:ms-btn--on={!data.muted}
    onclick={() => { data.muted = !data.muted; }}
    aria-label="Toggle mute"
    aria-pressed={data.muted}
    title="Mute"
  ></button>
  <button
    class="ms-btn ms-btn--solo"
    class:ms-btn--on={data.soloed}
    onclick={() => { data.soloed = !data.soloed; }}
    aria-label="Toggle solo"
    aria-pressed={data.soloed}
    title="Solo"
  ></button>
</div>

<style>
  .mute-solo {
    display: flex;
    gap: 5px;
    align-items: center;
    justify-content: center;
  }

  .ms-btn {
    width: 12px;
    height: 12px;
    border: none;
    background: #43454d;
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
    transition: background 0.1s;
  }

  .ms-btn--mute {
    border-radius: 50%;
  }

  .ms-btn--solo {
    border-radius: 4px;
  }

  .ms-btn--on {
    background: var(--accent, #e6b07f);
  }
</style>
