<script lang="ts">
  import ParamKnob from './ParamKnob.svelte';
  import Section from './Section.svelte';
  import { CUTOFF, ENV_DEPTH, RESONANCE, TIME } from '$lib/instrument/paramScales';
  import { defaultInstrument, FILTER_TYPES, type FilterType, type InstrumentSettings } from '$lib/instrument/types';
  import { hoverInfoAction } from '$lib/hoverInfo';

  let { inst, disabled }: { inst: InstrumentSettings; disabled: boolean } = $props();
  const d = defaultInstrument();
  const TYPE_LABELS: Record<FilterType, string> = { lowpass: 'LP', highpass: 'HP', bandpass: 'BP' };
</script>

<Section title="Filter">
  <div class="ftype" class:ftype--disabled={disabled} role="radiogroup" aria-label="Filter type">
    {#each FILTER_TYPES as t}
      <button
        class="ftype-btn"
        class:ftype-btn--on={inst.filterType === t}
        onclick={() => (inst.filterType = t)}
        role="radio"
        aria-checked={inst.filterType === t}
        use:hoverInfoAction={{ name: 'Filter type', value: t }}
      >{TYPE_LABELS[t]}</button>
    {/each}
  </div>
  <ParamKnob label="Cutoff" value={inst.cutoff} scale={CUTOFF} defaultValue={d.cutoff} onchange={(v) => (inst.cutoff = v)} {disabled} />
  <ParamKnob label="Reso" value={inst.resonance} scale={RESONANCE} defaultValue={d.resonance} onchange={(v) => (inst.resonance = v)} name="Resonance" {disabled} />
  <ParamKnob label="Env amt" value={inst.filterEnvDepth} scale={ENV_DEPTH} defaultValue={d.filterEnvDepth} onchange={(v) => (inst.filterEnvDepth = v)} name="Filter envelope amount" {disabled} />
  <ParamKnob label="Env att" value={inst.filterEnvAttack} scale={TIME} defaultValue={d.filterEnvAttack} onchange={(v) => (inst.filterEnvAttack = v)} name="Filter envelope attack" {disabled} />
  <ParamKnob label="Env dec" value={inst.filterEnvDecay} scale={TIME} defaultValue={d.filterEnvDecay} onchange={(v) => (inst.filterEnvDecay = v)} name="Filter envelope decay" {disabled} />
</Section>

<style>
  .ftype {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-right: 4px;
  }
  .ftype--disabled {
    opacity: 0.4;
    pointer-events: none;
  }
  .ftype-btn {
    width: 34px;
    height: 18px;
    font-size: 10px;
    font-weight: 600;
    border-radius: 5px;
    border: 1px solid var(--btn-border);
    background: var(--btn-bg);
    color: var(--btn-text);
    cursor: pointer;
  }
  .ftype-btn:hover { background: var(--btn-hover); }
  .ftype-btn--on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
  }
  .ftype-btn--on:hover { background: var(--accent-hover); }
</style>
