<script lang="ts">
  import Dial from '../Dial.svelte';
  import type { ParamScale } from '$lib/instrument/paramScales';

  // A labeled Dial over a real-valued parameter: the Dial itself works on
  // normalized 0..1 travel, `scale` maps to/from the real value.
  let {
    label,
    value,
    scale,
    defaultValue,
    onchange,
    name = '',
    size = 34,
    disabled = false,
  }: {
    label: string;
    value: number;
    scale: ParamScale;
    defaultValue: number;
    onchange: (v: number) => void;
    name?: string;
    size?: number;
    disabled?: boolean;
  } = $props();
</script>

<div class="pk" class:pk--disabled={disabled}>
  <Dial
    bind:value={() => scale.toNorm(value), (n) => onchange(scale.fromNorm(n))}
    defaultValue={scale.toNorm(defaultValue)}
    {size}
    showArc
    sensitivity={180}
    tooltipKey={label.toUpperCase()}
    format={(n) => scale.format(scale.fromNorm(n))}
    name={name || label}
  />
  <span class="pk-label">{label}</span>
  <span class="pk-value">{scale.format(value)}</span>
</div>

<style>
  .pk {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    min-width: 52px;
  }
  .pk--disabled {
    opacity: 0.4;
    pointer-events: none;
  }
  .pk-label {
    font-size: 10px;
    color: var(--main-text-muted);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    white-space: nowrap;
    max-width: 72px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pk-value {
    font-size: 10px;
    color: var(--main-text);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
</style>
