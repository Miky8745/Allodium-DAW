<script lang="ts">
  import ParamKnob from './ParamKnob.svelte';
  import { MACRO } from '$lib/instrument/paramScales';
  import { MACRO_COUNT, type InstrumentSettings } from '$lib/instrument/types';
  import type { SfzInstrument } from '$lib/instrument/sfz';

  // The preset's front panel: up to 8 macro knobs, one per labeled CC in
  // the .sfz (label_ccN), each moving whatever that preset maps to the CC.
  let { inst, preset, disabled }: { inst: InstrumentSettings; preset: SfzInstrument | null; disabled: boolean } = $props();

  let slots = $derived(Array.from({ length: MACRO_COUNT }, (_, i) => preset?.controls[i] ?? null));
</script>

<div class="macros">
  {#each slots as control, i}
    {#if control}
      <ParamKnob
        label={control.label}
        value={inst.macros[i]}
        scale={MACRO}
        defaultValue={control.value}
        onchange={(v) => (inst.macros[i] = v)}
        name="Macro: {control.label}"
        size={40}
        {disabled}
      />
    {:else}
      <div class="macro-empty" aria-hidden="true">
        <div class="macro-empty-dot"></div>
        <span>—</span>
      </div>
    {/if}
  {/each}
</div>

<style>
  .macros {
    display: grid;
    grid-template-columns: repeat(8, minmax(52px, 1fr));
    gap: 6px;
    padding: 10px 8px;
    background: rgba(0, 0, 0, 0.14);
    border: 1px solid var(--main-border);
    border-radius: 9px;
  }
  .macro-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    color: var(--main-text-muted);
    opacity: 0.4;
    font-size: 10px;
  }
  .macro-empty-dot {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    border: 1.5px dashed var(--main-border);
  }
</style>
