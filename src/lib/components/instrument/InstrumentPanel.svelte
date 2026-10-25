<script lang="ts">
  import type { ChannelData } from '$lib/types';
  import MacroRow from './MacroRow.svelte';
  import AmpSection from './AmpSection.svelte';
  import FilterSection from './FilterSection.svelte';
  import ParamKnob from './ParamKnob.svelte';
  import { TUNE } from '$lib/instrument/paramScales';
  import { loadPreset } from '$lib/instrument/presetLoader';
  import type { SfzInstrument } from '$lib/instrument/sfz';
  import { resetInstrument } from '$lib/instrument/applyPreset';
  import { usesPreset } from '$lib/instrument/channelSound';
  import { formatSourceLabel } from '$lib/sampleName';
  import { hoverInfoAction } from '$lib/hoverInfo';
  import { audioEngine } from '$lib/audioEngine';

  // Everything right of the preset browser: on/off, source, macros, amp
  // envelope and filter for one channel's instrument.
  let { channel }: { channel: ChannelData } = $props();

  let inst = $derived(channel.instrument);
  let preset = $state<SfzInstrument | null>(null);

  $effect(() => {
    const path = inst.presetPath;
    if (!path) { preset = null; return; }
    loadPreset(path).then(p => { if (channel.instrument.presetPath === path) preset = p; });
  });

  let source = $derived(
    usesPreset(channel) || channel.sampleFolder || channel.samplePath ? formatSourceLabel(channel)
    : 'No sound — pick a preset or drop a sample',
  );
  let detail = $derived(
    usesPreset(channel) && preset ? `${preset.regions.length} region${preset.regions.length === 1 ? '' : 's'} · ${preset.controls.length} macro${preset.controls.length === 1 ? '' : 's'}`
    : usesPreset(channel) ? 'Loading…'
    : 'Channel sample',
  );
  let disabled = $derived(!inst.enabled);

  function audition() {
    audioEngine.previewNote(channel, 60, undefined, 0.9);
  }
</script>

<div class="ip">
  <div class="ip-head">
    <button
      class="power"
      class:power--on={inst.enabled}
      onclick={() => (inst.enabled = !inst.enabled)}
      title={inst.enabled ? 'Instrument on' : 'Instrument off — the channel plays its sample as-is'}
      use:hoverInfoAction={{ name: 'Instrument', value: inst.enabled ? 'On' : 'Off' }}
      aria-pressed={inst.enabled}
    >
      <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
        <path d="M7 1.5v5"/>
        <path d="M3.8 3.6a5 5 0 1 0 6.4 0"/>
      </svg>
    </button>
    <div class="ip-src">
      <span class="ip-src-name">{source}</span>
      <span class="ip-src-detail">{detail}</span>
    </div>
    <ParamKnob label="Tune" value={inst.tune} scale={TUNE} defaultValue={0} onchange={(v) => (inst.tune = v)} size={26} {disabled} />
    <button class="ip-btn" onclick={audition} title="Play C5" use:hoverInfoAction={{ name: 'Audition', value: 'C5' }}>▶</button>
    <button class="ip-btn" onclick={() => resetInstrument(channel.id)} title="Reset the instrument (removes the preset)" use:hoverInfoAction={{ name: 'Init instrument', value: '' }}>Init</button>
  </div>

  {#if !inst.enabled}
    <p class="ip-hint">The instrument is off, so this channel plays its sample as-is. Turn it on, or pick a preset on the left.</p>
  {/if}

  <MacroRow {inst} {preset} {disabled} />
  <AmpSection {inst} {disabled} />
  <FilterSection {inst} {disabled} />
</div>

<style>
  .ip {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    overflow-y: auto;
    min-height: 0;
    height: 100%;
  }
  .ip-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .power {
    width: 26px;
    height: 26px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: 50%;
    border: 1px solid var(--btn-border);
    background: var(--btn-bg);
    color: var(--main-text-muted);
    cursor: pointer;
  }
  .power--on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
  }
  .ip-src {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .ip-src-name {
    font-size: 14px;
    font-weight: 600;
    color: var(--main-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ip-src-detail {
    font-size: 11px;
    color: var(--main-text-muted);
  }
  .ip-btn {
    height: 24px;
    min-width: 28px;
    padding: 0 8px;
    font-size: 11px;
    border-radius: 5px;
    border: 1px solid var(--btn-border);
    background: var(--btn-bg);
    color: var(--btn-text);
    cursor: pointer;
  }
  .ip-btn:hover { background: var(--btn-hover); }
  .ip-hint {
    margin: 0;
    font-size: 11px;
    color: var(--main-text-muted);
  }
</style>
