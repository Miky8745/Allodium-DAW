<script lang="ts">
  import ParamKnob from './ParamKnob.svelte';
  import Section from './Section.svelte';
  import EnvelopeGraph from './EnvelopeGraph.svelte';
  import { LEVEL, TIME } from '$lib/instrument/paramScales';
  import { defaultInstrument, type InstrumentSettings } from '$lib/instrument/types';

  let { inst, disabled }: { inst: InstrumentSettings; disabled: boolean } = $props();
  const d = defaultInstrument();
</script>

<Section title="Amp envelope">
  <ParamKnob label="Attack" value={inst.attack} scale={TIME} defaultValue={d.attack} onchange={(v) => (inst.attack = v)} {disabled} />
  <ParamKnob label="Decay" value={inst.decay} scale={TIME} defaultValue={d.decay} onchange={(v) => (inst.decay = v)} {disabled} />
  <ParamKnob label="Sustain" value={inst.sustain} scale={LEVEL} defaultValue={d.sustain} onchange={(v) => (inst.sustain = v)} {disabled} />
  <ParamKnob label="Release" value={inst.release} scale={TIME} defaultValue={d.release} onchange={(v) => (inst.release = v)} {disabled} />
  <EnvelopeGraph attack={inst.attack} decay={inst.decay} sustain={inst.sustain} release={inst.release} />
</Section>

