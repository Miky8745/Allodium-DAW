<script lang="ts">
  import type { ChannelData, FileDropDetail } from '$lib/types';
  import Dial from './Dial.svelte';
  import ScrollField from './ScrollField.svelte';
  import { formatChannelLabel } from '$lib/sampleName';
  import { channelHasSound } from '$lib/instrument/channelSound';
  import { channelPlugin } from '$lib/plugins/registry';
  import ChannelRowMenu from './ChannelRowMenu.svelte';
  import { auditionChannel } from '$lib/audition';
  import { channelStore } from '$lib/channelStore.svelte';
  import { channelLoopLength } from '$lib/channelLoop';
  import { formatPan } from '$lib/panFormat';
  import { hexToRgb } from '$lib/colorUtils';
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
  let isEmpty = $derived(!channelHasSound(channel) && !channel.plugin);

  // Sample field: double-click opens the assigned plugin, or the Piano Roll
  // when there is none.
  // Left press on the sample field plays the channel until release.
  function onSampleMousedown(e: MouseEvent) {
    if (e.button === 0) auditionChannel(channel);
  }

  function onSampleDblclick() {
    const plugin = channelPlugin(channel);
    if (plugin) plugin.open(channel.id);
    else onOpenPianoRoll();
  }

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
      auditionChannel(channel); // only the clicked step, not ones painted after
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

  // Right-click anywhere on the row (sample field included, steps excepted —
  // they erase on right-click, see stepMousedown) opens ChannelRowMenu.
  let contextMenu = $state<{ x: number; y: number } | null>(null);

  function openContextMenu(e: MouseEvent) {
    e.preventDefault();
    contextMenu = { x: e.clientX, y: e.clientY };
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
    onmousedown={onSampleMousedown}
    ondblclick={onSampleDblclick}
    role="button"
    tabindex="-1"
    aria-label="Sample field (press to hear, double-click to open, right-click for the channel menu)"
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
  <ChannelRowMenu {channel} x={contextMenu.x} y={contextMenu.y} onClose={() => (contextMenu = null)} {onDelete} />
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
</style>
