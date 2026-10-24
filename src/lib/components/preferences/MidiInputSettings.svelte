<script lang="ts">
  // Per-input-device processing, applied in this order (see
  // midiProcessing.ts): channel filter → key range → transpose → velocity.
  import type { MidiInputPrefs, VelocityCurve } from '$lib/preferencesTypes';
  import { pitchName } from '$lib/pianoroll/pitch';
  import ScrollField from '../ScrollField.svelte';
  import PrefRow from './PrefRow.svelte';
  import PrefSelect from './PrefSelect.svelte';
  import PrefSegmented from './PrefSegmented.svelte';
  import VelocityCurvePreview from './VelocityCurvePreview.svelte';

  let { prefs }: { prefs: MidiInputPrefs } = $props();

  const CHANNELS = [{ value: 0, label: 'Omni (all)' }, ...Array.from({ length: 16 }, (_, i) => ({ value: i + 1, label: `Channel ${i + 1}` }))];
  const CURVES: { value: VelocityCurve; label: string }[] = [
    { value: 'linear', label: 'Linear' },
    { value: 'soft', label: 'Soft' },
    { value: 'hard', label: 'Hard' },
    { value: 'custom', label: 'Custom' },
  ];

  function shiftOctave(dir: number) {
    prefs.transpose = Math.max(-48, Math.min(48, prefs.transpose + dir * 12));
  }
</script>

<PrefRow label="MIDI channel" hint="Ignore notes on other channels">
  <PrefSelect bind:value={prefs.channel} options={CHANNELS} width={130} />
</PrefRow>

<PrefRow label="Key range" hint="Notes outside are ignored (before transpose) — use for keyboard splits">
  <ScrollField bind:value={prefs.noteLow} min={0} max={127} step={1} width={36} tooltipKey="LOW" />
  <span class="note">{pitchName(prefs.noteLow)}</span>
  <span class="dash">–</span>
  <ScrollField bind:value={prefs.noteHigh} min={0} max={127} step={1} width={36} tooltipKey="HIGH" />
  <span class="note">{pitchName(prefs.noteHigh)}</span>
</PrefRow>

<PrefRow label="Transpose" hint="Semitones">
  <button class="mini-btn" onclick={() => shiftOctave(-1)} title="Octave down">−8va</button>
  <ScrollField bind:value={prefs.transpose} min={-48} max={48} step={1} width={40} label="st" tooltipKey="TRANSPOSE" />
  <button class="mini-btn" onclick={() => shiftOctave(1)} title="Octave up">+8va</button>
  <button class="mini-btn" onclick={() => (prefs.transpose = 0)} title="Reset transpose">0</button>
</PrefRow>

<PrefRow label="Pitch bend range" hint="Semitones at full wheel deflection (match your synth; 2 is the MIDI default)">
  <ScrollField bind:value={prefs.bendRange} min={0} max={24} step={1} width={40} label="st" tooltipKey="BEND" />
</PrefRow>

<PrefRow label="Fixed velocity" hint="0 = off (use what you play)">
  <ScrollField bind:value={prefs.fixedVelocity} min={0} max={127} step={1} width={40} tooltipKey="FIXED" />
</PrefRow>

{#if prefs.fixedVelocity === 0}
  <PrefRow label="Velocity curve">
    <PrefSegmented bind:value={prefs.velCurve} options={CURVES} />
  </PrefRow>
  {#if prefs.velCurve === 'custom'}
    <PrefRow label="Curve exponent" hint="<1 boosts soft playing, >1 needs harder hits">
      <ScrollField bind:value={prefs.velExponent} min={0.1} max={5} step={0.05} decimals={2} width={44} tooltipKey="EXP" />
    </PrefRow>
  {/if}
  <PrefRow label="Velocity offset" hint="Added after the curve">
    <ScrollField bind:value={prefs.velOffset} min={-127} max={127} step={1} width={40} tooltipKey="OFFSET" />
  </PrefRow>
  <PrefRow label="Velocity range" hint="Final clamp">
    <ScrollField bind:value={prefs.velMin} min={1} max={127} step={1} width={36} tooltipKey="MIN" />
    <span class="dash">–</span>
    <ScrollField bind:value={prefs.velMax} min={1} max={127} step={1} width={36} tooltipKey="MAX" />
  </PrefRow>
{/if}

<PrefRow label="Response" hint="In → out velocity">
  <VelocityCurvePreview {prefs} />
</PrefRow>

<style>
  .note {
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-size: 10.5px;
    color: var(--main-text-muted);
    width: 28px;
  }
  .dash { color: var(--main-text-muted); }
  .mini-btn {
    padding: 3px 7px;
    border-radius: 5px;
    border: 1px solid var(--btn-border);
    background: var(--btn-bg);
    color: var(--btn-text);
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-size: 10.5px;
    cursor: pointer;
  }
  .mini-btn:hover { background: var(--btn-hover); }
</style>
