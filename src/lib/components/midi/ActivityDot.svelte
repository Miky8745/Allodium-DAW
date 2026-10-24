<script lang="ts">
  // Blinks briefly every time `stamp` changes (a performance.now() value
  // from midiStore.activity). `connected` only changes its resting color.
  let { stamp = 0, connected = false }: { stamp?: number; connected?: boolean } = $props();

  const BLINK_MS = 90;
  let lit = $state(false);

  $effect(() => {
    if (!stamp) return;
    lit = true;
    const t = setTimeout(() => (lit = false), BLINK_MS);
    return () => clearTimeout(t);
  });
</script>

<span class="dot" class:dot--connected={connected} class:dot--on={lit}></span>

<style>
  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
    flex-shrink: 0;
    transition: background 0.15s, box-shadow 0.15s;
  }
  .dot--connected { background: #5d8a63; }
  .dot--on {
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent);
    transition: none;
  }
</style>
