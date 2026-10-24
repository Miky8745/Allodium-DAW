<script lang="ts">
  // Per-output-port settings. Which channels send to this port (and on
  // which MIDI channel) is set per channel, in the Channel Rack row's
  // right-click menu > MIDI out…
  import type { MidiOutputPrefs } from '$lib/preferencesTypes';
  import ScrollField from '../ScrollField.svelte';
  import PrefRow from './PrefRow.svelte';
  import PrefToggle from './PrefToggle.svelte';

  let { prefs }: { prefs: MidiOutputPrefs } = $props();
</script>

<PrefRow label="Latency offset" hint="Shift MIDI timing to line a hardware synth up with the audio (negative = earlier, limited by look-ahead)">
  <ScrollField bind:value={prefs.latencyMs} min={-100} max={500} step={1} width={52} label="ms" tooltipKey="OFFSET" />
</PrefRow>

<PrefRow label="Transpose" hint="Semitones">
  <ScrollField bind:value={prefs.transpose} min={-48} max={48} step={1} width={40} label="st" tooltipKey="TRANSPOSE" />
</PrefRow>

<PrefRow label="Velocity scale">
  <ScrollField
    bind:value={() => Math.round(prefs.velScale * 100), (v) => (prefs.velScale = v / 100)}
    min={10} max={200} step={5} width={52} label="%" tooltipKey="VEL"
  />
</PrefRow>

<PrefRow label="All notes off on stop" hint="Send CC123 on every channel when playback stops">
  <PrefToggle bind:checked={prefs.allNotesOffOnStop} label="All notes off on stop" />
</PrefRow>
