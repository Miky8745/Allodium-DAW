<script lang="ts">
  import Dial from '../Dial.svelte';

  // One labeled cell of the note properties panel's dial grid. `get`/`set`
  // rather than a bindable value, since the panel routes each write either
  // to the selected notes or to the new-note defaults. `mixed` (selected
  // notes disagree): `get` already returns the default, this just dims it
  // and reads "Mixed" — Dial only calls `set` once the user drags/resets.
  let {
    label,
    name,
    mixed = false,
    get,
    set,
    max = 1,
    defaultValue,
    sensitivity = 150,
    format,
  }: {
    label: string;
    name: string;
    mixed?: boolean;
    get: () => number;
    set: (v: number) => void;
    max?: number;
    defaultValue: number;
    sensitivity?: number;
    format: (v: number) => string;
  } = $props();
</script>

<div class="np-dial" class:np-dial--mixed={mixed}>
  <Dial bind:value={get, set} {max} {defaultValue} {sensitivity} format={mixed ? () => 'Mixed' : format} {name} tooltipKey={label} size={26} />
  <span class="np-dial-label">{mixed ? 'MIXED' : label}</span>
</div>

<style>
  .np-dial {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
  }

  .np-dial--mixed { opacity: 0.5; }

  .np-dial-label {
    font-size: 9px;
    letter-spacing: 0.06em;
    color: rgba(255,255,255,0.45);
    white-space: nowrap;
  }
</style>
