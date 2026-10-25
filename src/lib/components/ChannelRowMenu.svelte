<script lang="ts">
  import type { ChannelData } from '$lib/types';
  import ColorWheel from './ColorWheel.svelte';
  import ChannelMidiOutMenu from './ChannelMidiOutMenu.svelte';
  import { formatChannelLabel } from '$lib/sampleName';
  import { randomColorfulHex } from '$lib/colorUtils';
  import { portal } from '$lib/portal';
  import { PLUGINS, assignPlugin, removePlugin, pluginById } from '$lib/plugins/registry';

  // A Channel Rack row's single right-click menu (row and sample field alike):
  // plugins / open in Piano Roll / fill / rename / recolor / MIDI out / delete. Steps have their
  // own right-click meaning (erase step) and never open it. `menuView` lets
  // Rename/Recolor/MIDI out take over the same menu body instead of opening
  // a second portal/overlay.
  let { channel, x, y, onClose, onDelete, onOpenPianoRoll }: {
    channel: ChannelData;
    x: number;
    y: number;
    onClose: () => void;
    onDelete: () => void;
    onOpenPianoRoll: () => void;
  } = $props();

  let menuView = $state<'main' | 'rename' | 'recolor' | 'midi'>('main');

  // Plugins: picking one assigns it to the channel and opens it; the
  // assigned one is checked.
  function pickPlugin(id: string) {
    const channelId = channel.id;
    onClose();
    assignPlugin(channelId, id);
    pluginById(id)?.open(channelId);
  }

  function clearPlugin() {
    const channelId = channel.id;
    onClose();
    removePlugin(channelId);
  }

  function openPianoRoll() {
    onClose();
    onOpenPianoRoll();
  }

  function deleteChannel() {
    onClose();
    onDelete();
  }

  // Fill every Nth step (0, N, 2N, …), clearing the rest — a quick way to
  // sketch out a steady hi-hat/kick pattern. Operates on the channel's own
  // step array (not patternLength), same scope as the step buttons themselves.
  function fillEvery(n: number) {
    channel.steps = channel.steps.map((_, i) => i % n === 0);
    onClose();
  }

  // Rename: swaps the menu body for a single text field. Enter commits;
  // Escape or clicking away (the backdrop) cancels without committing —
  // deliberately no commit-on-blur here (unlike MixerChannelStrip's rename),
  // since removing a focused input from the DOM can itself dispatch a
  // trailing blur that would re-commit a just-cancelled edit (see
  // MixerChannelStrip.svelte's renameCancelled guard for that exact trap) —
  // simplest to just not wire blur at all for this short-lived menu widget.
  let nameDraft = $state('');
  let nameInputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (menuView === 'rename') nameInputEl?.select();
  });

  function openRename() {
    nameDraft = channel.name ?? formatChannelLabel(channel);
    menuView = 'rename';
  }

  function commitRename() {
    const trimmed = nameDraft.trim();
    channel.name = trimmed ? trimmed : null; // empty clears back to the auto-derived sample label
    onClose();
  }

  function renameKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') { e.preventDefault(); commitRename(); }
    else if (e.key === 'Escape') { e.preventDefault(); onClose(); }
  }

  // Recolor: swaps the menu body for a ColorWheel, live-writing
  // channel.color on every drag (same instant-apply convention as every
  // other control in the app) rather than needing a separate confirm step.
  // Seed a default before switching views so ColorWheel (which takes a
  // plain non-null hex) always has a concrete starting color to bind to.
  function openRecolor() {
    if (channel.color === null) channel.color = '#e6b07f';
    menuView = 'recolor';
  }

  function recolorRandomly() {
    channel.color = randomColorfulHex();
    onClose();
  }
</script>

<div class="ctx-root" use:portal>
  <div class="ctx-backdrop" onmousedown={onClose} oncontextmenu={(e) => { e.preventDefault(); onClose(); }} role="presentation"></div>
  <div class="ctx-menu" class:ctx-menu--wide={menuView === 'recolor' || menuView === 'midi'} style="left:{x}px; top:{y}px;" role="menu">
    {#if menuView === 'main'}
      <div class="ctx-heading">Plugins</div>
      {#each PLUGINS as p (p.id)}
        <button class="ctx-item ctx-plugin" role="menuitemradio" aria-checked={channel.plugin === p.id} onclick={() => pickPlugin(p.id)}>
          <span class="ctx-check">{channel.plugin === p.id ? '✓' : ''}</span>
          <span class="ctx-name">{p.name}</span>
          <span class="ctx-kind">{p.kind}</span>
        </button>
      {/each}
      {#if channel.plugin}
        <button class="ctx-item ctx-item--danger" role="menuitem" onclick={clearPlugin}>Remove plugin</button>
      {/if}
      <div class="ctx-sep"></div>
      <button class="ctx-item" role="menuitem" onclick={openPianoRoll}>Open in Piano Roll</button>
      <div class="ctx-sep"></div>
      <button class="ctx-item" role="menuitem" onclick={() => fillEvery(2)}>Fill every 2 steps</button>
      <button class="ctx-item" role="menuitem" onclick={() => fillEvery(4)}>Fill every 4 steps</button>
      <button class="ctx-item" role="menuitem" onclick={() => fillEvery(8)}>Fill every 8 steps</button>
      <div class="ctx-sep"></div>
      <button class="ctx-item" role="menuitem" onclick={openRename}>Rename</button>
      <button class="ctx-item" role="menuitem" onclick={openRecolor}>Recolor</button>
      <button class="ctx-item" role="menuitem" onclick={recolorRandomly}>Recolor randomly</button>
      <div class="ctx-sep"></div>
      <button class="ctx-item" role="menuitem" onclick={() => (menuView = 'midi')}>MIDI out…{channel.midiOutPort ? ` (ch ${channel.midiOutChannel})` : ''}</button>
      <div class="ctx-sep"></div>
      <button class="ctx-item ctx-item--danger" role="menuitem" onclick={deleteChannel}>Delete channel</button>
    {:else if menuView === 'rename'}
      <input
        class="ctx-input"
        type="text"
        bind:value={nameDraft}
        bind:this={nameInputEl}
        onkeydown={renameKeydown}
        placeholder="Channel name"
      />
    {:else if menuView === 'recolor' && channel.color}
      <ColorWheel bind:value={channel.color} />
      <button class="ctx-item ctx-done" role="menuitem" onclick={onClose}>Done</button>
    {:else if menuView === 'midi'}
      <ChannelMidiOutMenu {channel} onDone={onClose} />
    {/if}
  </div>
</div>

<style>
  .ctx-root {
    position: fixed;
    inset: 0;
    z-index: 10000;
  }

  .ctx-backdrop {
    position: absolute;
    inset: 0;
  }

  .ctx-menu {
    position: fixed;
    background: #272a33;
    border: 1px solid var(--explorer-border, #34373f);
    border-radius: 8px;
    box-shadow: 0 6px 18px rgba(0,0,0,0.5);
    min-width: 170px;
    padding: 3px;
    display: flex;
    flex-direction: column;
  }

  .ctx-menu--wide {
    min-width: 190px;
  }

  .ctx-heading {
    padding: 4px 10px 3px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--main-text-muted);
  }

  .ctx-item {
    background: transparent;
    border: none;
    color: rgba(255,255,255,0.7);
    font-size: 12px;
    text-align: left;
    padding: 5px 10px;
    border-radius: 5px;
    cursor: pointer;
    transition: background 0.08s, color 0.08s;
  }

  .ctx-item:hover {
    background: rgba(255,255,255,0.07);
    color: rgba(255,255,255,0.95);
  }

  .ctx-item--danger:hover {
    background: rgba(220,50,50,0.18);
    color: #f08484;
  }

  .ctx-plugin {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .ctx-check {
    width: 12px;
    color: var(--accent);
    flex-shrink: 0;
  }

  .ctx-name { flex: 1; }

  .ctx-kind {
    font-size: 10px;
    color: var(--main-text-muted);
  }

  .ctx-sep {
    height: 1px;
    margin: 4px 6px;
    background: rgba(255,255,255,0.08);
  }

  .ctx-input {
    width: 160px;
    margin: 2px;
    padding: 5px 8px;
    font-size: 12px;
    color: var(--main-text);
    background: rgba(0,0,0,0.25);
    border: 1px solid rgba(255,255,255,0.15);
    border-radius: 5px;
    outline: none;
  }

  .ctx-input:focus {
    border-color: var(--accent);
  }

  .ctx-done {
    margin-top: 2px;
    text-align: center;
    color: var(--accent);
  }
</style>
