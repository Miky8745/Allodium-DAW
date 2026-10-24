<script lang="ts">
  // Toolbar MIDI activity light — blinks on any MIDI message in or out.
  // Hovering it shows the last incoming message in the info panel.
  import { midiStore } from '$lib/midi/midiStore.svelte';
  import { hoverInfoAction } from '$lib/hoverInfo';

  const BLINK_MS = 90;

  let lit = $state(false);

  $effect(() => {
    if (midiStore.lastActivity === 0) return;
    lit = true;
    const t = setTimeout(() => (lit = false), BLINK_MS);
    return () => clearTimeout(t);
  });
</script>

<div class="midi-led" use:hoverInfoAction={{ name: 'MIDI activity', value: midiStore.lastMessage ?? 'No MIDI received yet' }}>
  <span class="dot" class:dot--on={lit}></span>
  <span class="label">MIDI</span>
</div>

<style>
  .midi-led {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 3px 8px;
    border-radius: 7px;
    background: var(--btn-bg);
    border: 1px solid var(--btn-border);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.12);
    transition: background 0.15s, box-shadow 0.15s;
  }
  .dot--on {
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent);
    transition: none;
  }
  .label {
    font-size: 9px;
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.3);
  }
</style>
