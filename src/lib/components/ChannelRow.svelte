<script lang="ts">
  import type { ChannelData, FileDropDetail } from '$lib/types';
  import Dial from './Dial.svelte';
  import ScrollField from './ScrollField.svelte';
  import ColorWheel from './ColorWheel.svelte';
  import ChannelMidiOutMenu from './ChannelMidiOutMenu.svelte';
  import { formatChannelLabel } from '$lib/sampleName';
  import { channelStore } from '$lib/channelStore.svelte';
  import { channelLoopLength } from '$lib/channelLoop';
  import { formatPan } from '$lib/panFormat';
  import { hexToRgb, randomColorfulHex } from '$lib/colorUtils';
  import { portal } from '$lib/portal';
  import MiniNoteRoll from './pianoroll/MiniNoteRoll.svelte';

  let {
    channel,
    nameColWidth,
    selected,
    activeStep = -1,
    onSelect,
    onStepChange,
    onSampleDrop,
    onOpenPianoRoll,
    onDelete,
  }: {
    channel:      ChannelData;
    nameColWidth: number;
    selected:     boolean;
    activeStep?:  number;
    onSelect:     (e: MouseEvent) => void;
    onStepChange: (step: number, active: boolean) => void;
    onSampleDrop: (drop: FileDropDetail) => void;
    onOpenPianoRoll: () => void;
    onDelete:     () => void;
  } = $props();

  // Paint mode shared across all rows via module-level state
  // (reset on global pointerup)

  let dragOver = $state(false);
  let dropEl = $state<HTMLElement | null>(null);

  $effect(() => {
    if (!dropEl) return;
    const el = dropEl;
    function onEnter() { dragOver = true; }
    function onLeave() { dragOver = false; }
    function onDrop(e: Event) {
      dragOver = false;
      onSampleDrop((e as CustomEvent<FileDropDetail>).detail);
    }
    el.addEventListener('filedragenter', onEnter);
    el.addEventListener('filedragleave', onLeave);
    el.addEventListener('filedrop', onDrop);
    return () => {
      el.removeEventListener('filedragenter', onEnter);
      el.removeEventListener('filedragleave', onLeave);
      el.removeEventListener('filedrop', onDrop);
    };
  });

  let sampleLabel = $derived(formatChannelLabel(channel));
  let isEmpty = $derived(channel.samplePath === null && channel.sampleFolder === null);

  // Mute dot: plain click toggles mute (unchanged); Ctrl+click toggles solo
  // instead — single-solo (channelStore.toggleSolo), so soloing a channel
  // un-solos whichever other one was soloed, and Ctrl+clicking the
  // already-soloed channel again leaves nothing soloed (the "reverse").
  // Channel Rack only — unrelated to the Mixer's own separate mute/solo.
  function toggleMuteOrSolo(e: MouseEvent) {
    if (e.ctrlKey) channelStore.toggleSolo(channel.id);
    else channel.muted = !channel.muted;
  }

  // Custom sample-box color (right-click > Recolor/Recolor randomly) — only
  // the sample-name box is tinted, never the whole row. When set, pick a
  // dark-or-light label color off its luminance so the name stays readable
  // against an arbitrary hue instead of assuming the box is always dark.
  let dropTextColor = $derived.by(() => {
    if (!channel.color) return null;
    const [r, g, b] = hexToRgb(channel.color);
    const luminance = (r * 299 + g * 587 + b * 114) / 1000;
    return luminance < 140 ? 'rgba(255,255,255,0.92)' : 'rgba(20,20,20,0.92)';
  });

  // This channel may loop independently at a shorter length than the shared
  // pattern (e.g. a 1-bar kick inside a 4-bar pattern) — activeStep is fed
  // the raw, whole-pattern step, so it must be wrapped to this channel's own
  // loop length before comparing against the fixed 0-15 step buttons/preview,
  // otherwise the highlight only shows during the pattern's first cycle.
  let chLoopLength = $derived(channelLoopLength(channel, channelStore.activeLength, channelStore.barLength));
  let wrappedStep = $derived(activeStep >= 0 ? activeStep % chLoopLength : -1);

  // Sequencer paint
  let painting = $state<boolean | null>(null); // true=activate, false=deactivate

  function stepMousedown(e: MouseEvent, i: number) {
    e.preventDefault();
    if (e.button === 2) {
      painting = false;
      onStepChange(i, false);
    } else if (e.button === 0) {
      painting = true;
      onStepChange(i, true);
    }
  }

  function stepPointerenter(e: PointerEvent, i: number) {
    if (painting === null) return;
    // only when a button is held
    if (e.buttons === 0) { painting = null; return; }
    onStepChange(i, painting);
  }

  function globalPointerup() {
    painting = null;
  }

  // Step color group: 0,2 = grey; 1,3 = orange/red
  function stepGroup(i: number) {
    return Math.floor(i / 4) % 2; // 0 or 1
  }

  // Mini piano-roll preview (replaces the step buttons for note-based channels)

  // Right-click context menu: fill/rename/recolor/delete. Steps have their
  // own right-click meaning (erase step, see stepMousedown) and stop this
  // from firing when right-clicked — see their oncontextmenu handler below.
  // `menuView` lets Rename/Recolor take over the same menu body instead of
  // opening a second portal/overlay.
  let contextMenu = $state<{ x: number; y: number } | null>(null);
  let menuView = $state<'main' | 'rename' | 'recolor' | 'midi'>('main');

  function openContextMenu(e: MouseEvent) {
    e.preventDefault();
    contextMenu = { x: e.clientX, y: e.clientY };
    menuView = 'main';
  }

  function closeContextMenu() {
    contextMenu = null;
    menuView = 'main';
  }

  function deleteChannel() {
    closeContextMenu();
    onDelete();
  }

  // Fill every Nth step (0, N, 2N, …), clearing the rest — a quick way to
  // sketch out a steady hi-hat/kick pattern. Operates on the channel's own
  // fixed 16-button step array (not patternLength), same scope as the step
  // buttons themselves.
  function fillEvery(n: number) {
    channel.steps = channel.steps.map((_, i) => i % n === 0);
    closeContextMenu();
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
    closeContextMenu();
  }

  function renameKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') { e.preventDefault(); commitRename(); }
    else if (e.key === 'Escape') { e.preventDefault(); closeContextMenu(); }
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
    closeContextMenu();
  }
</script>

<svelte:window onpointerup={globalPointerup} />

<div class="ch-row" class:ch-row--selected={selected} oncontextmenu={openContextMenu} role="row" tabindex="-1">
  <!-- Mute dot -->
  <button
    class="mute-dot"
    class:muted={channel.muted}
    class:soloed={channel.soloed}
    onclick={toggleMuteOrSolo}
    aria-label="Toggle mute (Ctrl+click: solo)"
    aria-pressed={channel.muted}
    title="Mute (Ctrl+click: Solo)"
  ></button>

  <!-- Pan dial -->
  <Dial bind:value={channel.pan} defaultValue={0.5} size={24} sensitivity={150} tooltipKey="PAN" format={formatPan} name="Pan dial" />

  <!-- Volume dial: 0-200% (100%/unity gain sits at the top), half-circle
       sweeping left→top→right so max lands on the right, not the bottom. -->
  <Dial
    bind:value={channel.volume}
    defaultValue={0.8}
    max={2}
    sweepStart={270}
    sweepDegrees={180}
    size={24}
    sensitivity={150}
    tooltipKey="VOLUME"
    name="Volume dial"
  />

  <!-- Mixer track -->
  <ScrollField bind:value={channel.mixerTrack} min={0} max={125} width={30} tooltipKey="MIXER TRACK" name="Mixer track" />

  <!-- Sample drop zone -->
  <div
    class="sample-drop"
    class:dragover={dragOver}
    class:empty={isEmpty}
    style="width:{nameColWidth}px; min-width:{nameColWidth}px; max-width:{nameColWidth}px; {channel.color ? `background:${channel.color};` : ''}"
    data-sample-drop
    bind:this={dropEl}
    ondblclick={onOpenPianoRoll}
    role="region"
    aria-label="Sample drop zone"
  >
    <span class="sample-name" style={dropTextColor ? `color:${dropTextColor};` : ''}>{sampleLabel}</span>
  </div>

  <!-- Select toggle -->
  <button
    class="sel-btn"
    class:sel-btn--on={selected}
    onclick={onSelect}
    aria-label="Select channel"
    title="Select"
  ></button>

  <!-- Sequencer buttons / mini piano-roll preview -->
  <div class="steps">
    {#if channel.notes.length > 0}
      <MiniNoteRoll notes={channel.notes} patternLength={chLoopLength} activeStep={wrappedStep} />
    {:else}
      {#each channel.steps as active, i}
        {@const grp = stepGroup(i)}
        <button
          class="step"
          class:step--on={active}
          class:step--grey={grp === 0}
          class:step--orange={grp === 1}
          class:step--playing={i === wrappedStep}
          onmousedown={(e) => stepMousedown(e, i)}
          onpointerenter={(e) => stepPointerenter(e, i)}
          oncontextmenu={(e) => { e.preventDefault(); e.stopPropagation(); }}
          aria-label="Step {i + 1}"
          aria-pressed={active}
        ></button>
      {/each}
    {/if}
  </div>
</div>

{#if contextMenu}
  <div class="ctx-root" use:portal>
    <div class="ctx-backdrop" onmousedown={closeContextMenu} oncontextmenu={(e) => e.preventDefault()} role="presentation"></div>
    <div class="ctx-menu" class:ctx-menu--wide={menuView === 'recolor' || menuView === 'midi'} style="left:{contextMenu.x}px; top:{contextMenu.y}px;" role="menu">
      {#if menuView === 'main'}
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
        <button class="ctx-item ctx-done" role="menuitem" onclick={closeContextMenu}>Done</button>
      {:else if menuView === 'midi'}
        <ChannelMidiOutMenu {channel} onDone={closeContextMenu} />
      {/if}
    </div>
  </div>
{/if}

<style>
  .ch-row {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 8px;
    border-bottom: 1px solid rgba(255,255,255,0.05);
    background: transparent;
    transition: background 0.06s;
  }

  .ch-row:hover {
    background: rgba(255,255,255,0.03);
  }

  .ch-row--selected {
    background: rgba(230,176,127, 0.08);
  }

  /* Mute dot */
  .mute-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: none;
    background: var(--accent);
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.1s, box-shadow 0.1s;
    padding: 0;
  }

  .mute-dot.muted {
    background: #43454d;
  }

  /* Soloed (Ctrl+click) — a glowing ring, not a color swap, since the app
     only has one accent color token and the dot's own fill already carries
     the mute/unmute meaning. Shows on top of either fill, including the
     muted+soloed combination (explicit mute still wins on audibility). */
  .mute-dot.soloed {
    box-shadow: 0 0 0 2px var(--accent), 0 0 6px 1px rgba(230, 176, 127, 0.55);
  }

  /* Sample drop */
  .sample-drop {
    height: 28px;
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 5px;
    background: rgba(0,0,0,0.2);
    display: flex;
    align-items: center;
    padding: 0 6px;
    overflow: hidden;
    cursor: default;
    flex-shrink: 0;
    transition: border-color 0.1s, background 0.1s;
  }

  .sample-drop.dragover {
    border-color: var(--accent);
    background: rgba(230,176,127,0.12);
  }

  .sample-name {
    font-size: 11px;
    color: var(--main-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sample-drop.empty .sample-name {
    color: var(--main-text-muted);
    font-style: italic;
  }

  /* Select button */
  .sel-btn {
    width: 16px;
    height: 16px;
    border-radius: 4px;
    border: 1px solid rgba(255,255,255,0.15);
    background: transparent;
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.1s;
    padding: 0;
  }

  .sel-btn--on {
    background: var(--accent);
    border-color: var(--accent);
  }

  /* Steps */
  .steps {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }

  .step {
    width: 24px;
    height: 28px;
    border-radius: 5px;
    border: none;
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
    transition: background 0.06s;
  }

  .step--grey     { background: #30323b; }
  .step--orange   { background: #2e1f14; }

  .step--grey.step--on   { background: #7d7f85; }
  .step--orange.step--on { background: var(--accent); }

  .step:hover:not(.step--on) { filter: brightness(1.4); }

  .step--playing { outline: 2px solid rgba(255,255,255,0.55); outline-offset: -2px; }

  /* Context menu */
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
    min-width: 140px;
    padding: 3px;
    display: flex;
    flex-direction: column;
  }

  .ctx-menu--wide {
    min-width: 190px;
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
