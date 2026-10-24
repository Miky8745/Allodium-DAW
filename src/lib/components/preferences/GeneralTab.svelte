<script lang="ts">
  import { preferences } from '$lib/preferencesStore.svelte';
  import { audioEngine } from '$lib/audioEngine';
  import type { LatencyHint, MonitorSource } from '$lib/preferencesTypes';
  import ScrollField from '../ScrollField.svelte';
  import PrefSection from './PrefSection.svelte';
  import PrefRow from './PrefRow.svelte';
  import PrefSelect from './PrefSelect.svelte';
  import PrefToggle from './PrefToggle.svelte';
  import PrefSegmented from './PrefSegmented.svelte';

  let g = $derived(preferences.general);

  const SCALES = [0.75, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2].map(v => ({ value: v, label: `${Math.round(v * 100)}%` }));
  const LATENCY: { value: LatencyHint; label: string }[] = [
    { value: 'interactive', label: 'Interactive (lowest)' },
    { value: 'balanced', label: 'Balanced' },
    { value: 'playback', label: 'Playback (safest)' },
  ];
  const MONITOR_SOURCES: { value: MonitorSource; label: string }[] = [
    { value: 'app', label: 'App audio' },
    { value: 'system', label: 'System audio' },
  ];
  const RATES = [
    { value: 0, label: 'Auto (device)' },
    { value: 44100, label: '44.1 kHz' },
    { value: 48000, label: '48 kHz' },
    { value: 96000, label: '96 kHz' },
  ];

  // Read-only readout of what the AudioContext actually got. Refreshed
  // shortly after any latency/sample-rate change (the rebuild is async)
  // and once a second while the tab is open, since outputLatency can settle
  // after the stream starts.
  let latency = $state(audioEngine.getLatencyInfo());
  $effect(() => {
    void g.latencyHint; void g.sampleRate;
    const id = setInterval(() => (latency = audioEngine.getLatencyInfo()), 1000);
    const t = setTimeout(() => (latency = audioEngine.getLatencyInfo()), 150);
    return () => { clearInterval(id); clearTimeout(t); };
  });

  const ms = (s: number) => `${(s * 1000).toFixed(1)} ms`;
</script>

<div class="tab">
  <PrefSection title="Interface">
    <PrefRow label="GUI scale" hint="Zooms the whole interface">
      <PrefSegmented bind:value={g.guiScale} options={SCALES} />
    </PrefRow>
  </PrefSection>

  <PrefSection title="Audio">
    <PrefRow label="Output device" hint="Uses the system default output (selection not supported by the Linux webview yet)">
      <span class="muted">System default</span>
    </PrefRow>
    <PrefRow label="Latency mode" hint="Lower = more responsive, higher = fewer dropouts. Rebuilds audio (stops playback).">
      <PrefSelect bind:value={g.latencyHint} options={LATENCY} width={170} />
    </PrefRow>
    <PrefRow label="Sample rate" hint="Rebuilds audio (stops playback)">
      <PrefSelect bind:value={g.sampleRate} options={RATES} width={170} />
    </PrefRow>
    <PrefRow label="Current latency">
      {#if latency}
        <span class="readout">base {ms(latency.baseLatency)} · output {ms(latency.outputLatency)} · {latency.sampleRate} Hz</span>
      {:else}
        <span class="muted">Audio not started yet</span>
      {/if}
    </PrefRow>
    <PrefRow label="Audio monitor source" hint="What the Toolbar's oscilloscope/spectrum shows. System audio = everything your computer is playing.">
      <PrefSelect bind:value={g.monitorSource} options={MONITOR_SOURCES} width={170} />
    </PrefRow>
    <PrefRow label="Scheduler look-ahead" hint="Advanced — how far ahead notes are scheduled">
      <ScrollField bind:value={g.lookaheadMs} min={20} max={500} step={5} width={60} label="ms" tooltipKey="LOOKAHEAD" />
    </PrefRow>
    <PrefRow label="Scheduler tick" hint="Advanced — must stay well below the look-ahead">
      <ScrollField bind:value={g.tickMs} min={5} max={100} step={1} width={60} label="ms" tooltipKey="TICK" />
    </PrefRow>
  </PrefSection>

  <PrefSection title="Saving & history">
    <PrefRow label="Autosave" hint="Also in File > Autosave">
      <PrefToggle bind:checked={g.autosaveEnabled} label="Autosave" />
    </PrefRow>
    <PrefRow label="Save after idle" hint="Delay after the last edit before an undo step / autosave">
      <ScrollField bind:value={g.autosaveDelayMs} min={200} max={10000} step={100} width={60} label="ms" tooltipKey="IDLE" />
    </PrefRow>
    <PrefRow label="Undo history size">
      <ScrollField bind:value={g.undoLimit} min={10} max={2000} step={10} width={60} label="steps" tooltipKey="UNDO" />
    </PrefRow>
  </PrefSection>

  <PrefSection title="Startup & new projects">
    <PrefRow label="Reopen last project on launch">
      <PrefToggle bind:checked={g.reopenLastProject} label="Reopen last project" />
    </PrefRow>
    <PrefRow label="Default tempo" hint="For newly created projects">
      <ScrollField bind:value={g.defaultBpm} min={20} max={999} step={1} width={60} label="BPM" tooltipKey="BPM" />
    </PrefRow>
    <PrefRow label="Default time signature" hint="Beats per bar (quarter notes)">
      <ScrollField bind:value={g.defaultBeatsPerBar} min={1} max={16} step={1} width={48} label="/4" tooltipKey="SIG" />
    </PrefRow>
  </PrefSection>

  <PrefSection title="Piano roll">
    <PrefRow label="Preview volume" hint="Click/place/move audition">
      <ScrollField
        bind:value={() => Math.round(g.previewVolume * 100), (v) => (g.previewVolume = v / 100)}
        min={0} max={100} step={1} width={52} label="%" tooltipKey="VOL"
      />
    </PrefRow>
    <PrefRow label="Preview length">
      <ScrollField bind:value={g.previewLengthMs} min={20} max={2000} step={10} width={60} label="ms" tooltipKey="LEN" />
    </PrefRow>
    <PrefRow label="Typing keyboard velocity" hint="Velocity of notes played on the computer keyboard">
      <ScrollField bind:value={g.qwertyVelocity} min={1} max={127} step={1} width={48} tooltipKey="VEL" />
    </PrefRow>
  </PrefSection>
</div>

<style>
  .tab {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .muted {
    font-size: 11.5px;
    color: var(--main-text-muted);
  }
  .readout {
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-size: 11px;
    color: var(--main-text);
  }
</style>
