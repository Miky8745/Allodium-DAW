<script lang="ts">
  import { preferences } from '$lib/preferencesStore.svelte';
  import { midiSystem } from '$lib/midi/midiSystem.svelte';
  import PrefSection from './PrefSection.svelte';
  import MidiDeviceList from './MidiDeviceList.svelte';
  import MidiInputSettings from './MidiInputSettings.svelte';
  import MidiOutputSettings from './MidiOutputSettings.svelte';
  import MidiMonitor from './MidiMonitor.svelte';

  let selectedInput = $state<string | null>(null);
  let selectedOutput = $state<string | null>(null);
  let rescanning = $state(false);

  let inputPrefs = $derived(selectedInput ? preferences.findInput(selectedInput) : undefined);
  let outputPrefs = $derived(selectedOutput ? preferences.findOutput(selectedOutput) : undefined);

  async function rescan() {
    rescanning = true;
    try { await midiSystem.rescan(); } finally { rescanning = false; }
  }
</script>

<div class="tab">
  <div class="intro">
    <p>
      Enabled inputs play the channel selected in the Channel Rack (and glow in the Piano Roll).
      Devices are remembered by name and reconnect automatically when plugged back in.
    </p>
    <button class="btn" onclick={rescan} disabled={rescanning}>{rescanning ? 'Scanning…' : 'Rescan'}</button>
  </div>

  <PrefSection title="Inputs">
    <MidiDeviceList kind="input" bind:selected={selectedInput} />
    {#if inputPrefs}
      <div class="settings">
        <div class="settings-title">{inputPrefs.name}</div>
        <MidiInputSettings prefs={inputPrefs} />
      </div>
    {/if}
  </PrefSection>

  <PrefSection title="Outputs">
    <MidiDeviceList kind="output" bind:selected={selectedOutput} />
    {#if outputPrefs}
      <div class="settings">
        <div class="settings-title">{outputPrefs.name}</div>
        <MidiOutputSettings prefs={outputPrefs} />
      </div>
    {/if}
    <p class="note">
      Only piano-roll notes are sent (never step-sequencer hits). Pick a port per channel in the
      Channel Rack: right-click a channel › MIDI out…
    </p>
  </PrefSection>

  <PrefSection title="Monitor">
    <MidiMonitor />
  </PrefSection>
</div>

<style>
  .tab {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .intro {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }
  .intro p, .note {
    flex: 1;
    margin: 0;
    font-size: 11.5px;
    line-height: 1.45;
    color: var(--main-text-muted);
  }
  .note { padding: 4px 2px 2px; }
  .settings {
    margin-top: 6px;
    padding-top: 8px;
    border-top: 1px solid var(--main-border);
  }
  .settings-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--accent);
    margin-bottom: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .btn {
    padding: 5px 12px;
    border-radius: 7px;
    border: 1px solid var(--btn-border);
    background: var(--btn-bg);
    color: var(--btn-text);
    font-family: inherit;
    font-size: 12px;
    cursor: pointer;
    flex-shrink: 0;
  }
  .btn:hover:not(:disabled) { background: var(--btn-hover); }
  .btn:disabled { opacity: 0.6; cursor: default; }
</style>
