<script lang="ts">
  // Inputs or outputs: every port the backend currently sees, plus any
  // remembered (in Preferences) that are unplugged right now — so a
  // device's settings stay visible/editable while it's away. Click a row to
  // edit its settings; the switch enables/disables it.
  import { preferences } from '$lib/preferencesStore.svelte';
  import { midiStore } from '$lib/midi/midiStore.svelte';
  import ActivityDot from '../midi/ActivityDot.svelte';
  import PrefToggle from './PrefToggle.svelte';

  let { kind, selected = $bindable(null) }: { kind: 'input' | 'output'; selected: string | null } = $props();

  let present = $derived(kind === 'input' ? midiStore.ports.inputs : midiStore.ports.outputs);
  let connected = $derived(kind === 'input' ? midiStore.ports.connectedInputs : midiStore.ports.connectedOutputs);
  let remembered = $derived((kind === 'input' ? preferences.inputs : preferences.outputs).map(p => p.name));
  let names = $derived([...present, ...remembered.filter(n => !present.includes(n))]);

  function prefsFor(name: string) {
    return kind === 'input' ? preferences.inputPrefs(name) : preferences.outputPrefs(name);
  }

  function isEnabled(name: string) {
    return (kind === 'input' ? preferences.findInput(name) : preferences.findOutput(name))?.enabled ?? false;
  }

  function status(name: string) {
    if (connected.includes(name)) return 'Connected';
    if (!present.includes(name)) return isEnabled(name) ? 'Unplugged — will reconnect' : 'Unplugged';
    return isEnabled(name) ? 'Connecting…' : 'Available';
  }

  function select(name: string) {
    prefsFor(name); // create its settings entry on first touch
    selected = name;
  }
</script>

<div class="list">
  {#each names as name (name)}
    <div
      class="row"
      class:row--selected={selected === name}
      class:row--missing={!present.includes(name)}
      role="button"
      tabindex="0"
      onclick={() => select(name)}
      onkeydown={(e) => { if (e.key === 'Enter') select(name); }}
    >
      <ActivityDot stamp={midiStore.activity.get(name) ?? 0} connected={connected.includes(name)} />
      <div class="info">
        <span class="name" title={name}>{name}</span>
        <span class="status">{status(name)}</span>
      </div>
      <!-- stopPropagation so toggling doesn't also count as a row click -->
      <div class="tgl" role="presentation" onclick={(e) => e.stopPropagation()}>
        <PrefToggle
          bind:checked={() => isEnabled(name), (v) => { prefsFor(name).enabled = v; }}
          label={`Enable ${name}`}
        />
      </div>
    </div>
  {:else}
    <div class="empty">No MIDI {kind}s found. Plug a device in — it'll show up within a couple of seconds.</div>
  {/each}
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 180px;
    overflow: auto;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: 7px;
    cursor: pointer;
    border: 1px solid transparent;
  }
  .row:hover { background: var(--btn-hover); }
  .row--selected {
    background: rgba(230, 176, 127, 0.08);
    border-color: rgba(230, 176, 127, 0.4);
  }
  .row--missing .name { color: var(--main-text-muted); }
  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .name {
    font-size: 12.5px;
    color: var(--main-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .status {
    font-size: 10.5px;
    color: var(--main-text-muted);
  }
  .empty {
    font-size: 11.5px;
    color: var(--main-text-muted);
    padding: 8px 4px;
  }
</style>
