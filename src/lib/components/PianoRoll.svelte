<script lang="ts">
  import { channelStore } from '$lib/channelStore.svelte';
  import { playback } from '$lib/playbackStore.svelte';
  import { channelLoopLength } from '$lib/channelLoop';
  import { formatChannelLabel } from '$lib/sampleName';
  import { KEY_COL_W, RULER_H, LANE_H } from '$lib/pianoroll/pitch';
  import FloatingWindow, { type WorkspaceBounds } from './FloatingWindow.svelte';
  import PianoRollHeader, { type PianoRollTool } from './pianoroll/PianoRollHeader.svelte';
  import PianoKeys from './pianoroll/PianoKeys.svelte';
  import PianoRollRuler from './pianoroll/PianoRollRuler.svelte';
  import NoteGrid from './pianoroll/NoteGrid.svelte';
  import { liveNotes } from '$lib/midi/liveNotes.svelte';
  import VelocityLane from './pianoroll/VelocityLane.svelte';
  import PianoRollOverview from './pianoroll/PianoRollOverview.svelte';
  import NotePropertiesPanel from './pianoroll/NotePropertiesPanel.svelte';

  let {
    show = $bindable(),
    workspaceBounds,
    showPianoRollOverview = $bindable(false),
  }: { show: boolean; workspaceBounds: WorkspaceBounds; showPianoRollOverview?: boolean } = $props();

  let tool = $state<PianoRollTool>('draw');
  let selectedNoteIds = $state(new Set<number>());
  let gridScrollLeft = $state(0);
  let gridScrollTop = $state(0);
  let gridViewportWidth = $state(0);
  let noteGridRef: NoteGrid | undefined = $state();
  let highlightPitches = $state<Set<number>>(new Set());
  let showNoteProps = $state(true);

  let channel = $derived(channelStore.selectedChannel);
  let channelName = $derived(channel ? formatChannelLabel(channel) : 'No channel');

  // playback.currentStep/currentStepFraction are the absolute, unwrapped
  // position within channelStore.activeLength — correct for a channel whose
  // content fills the whole active region, but a channel that loops on a
  // shorter length of its own (see channelLoop.ts) would otherwise show a
  // playhead sweeping across empty canvas for most of the active region
  // instead of cycling back over its own notes, which reads as if it's
  // tracking some other channel's timing. Wrapping to this channel's own
  // loop length here (mirroring audioEngine.tick()'s chStep) keeps the
  // Piano Roll's playhead in sync with what's actually audible for it.
  let chLoopLength = $derived(
    channel ? channelLoopLength(channel, channelStore.activeLength, channelStore.barLength) : 1
  );
  let wrappedStep = $derived(playback.currentStep >= 0 ? playback.currentStep % chLoopLength : -1);
  // Song mode drives an absolute song-wide step counter, not a
  // pattern-relative one — wrapping it by this channel's own pattern-local
  // loop length would be musically meaningless, so the playhead only shows
  // in pattern mode (matching ChannelRack's own play button, which forces
  // pattern mode before it starts playback — see handleTogglePlay below).
  let wrappedStepFraction = $derived(
    playback.isPlaying && playback.transportMode === 'pattern' ? playback.currentStepFraction % chLoopLength : -1
  );

  // Opening the Piano Roll with nothing selected (toolbar button before any
  // channel was ever picked, or after the selected channel was deleted)
  // falls back to the first channel. It always shows the live pattern's
  // content, so "the currently selected pattern" needs nothing extra.
  $effect(() => {
    if (show && !channelStore.selectedChannel && channelStore.channels.length > 0) {
      channelStore.selectedChannelId = channelStore.channels[0].id;
    }
  });

  // Selection is per-channel edit state — clear it when the focused channel changes.
  $effect(() => {
    channelStore.selectedChannelId;
    selectedNoteIds = new Set();
  });

  // Auto-extend: as soon as any channel has a note reaching into the pattern's
  // last bar, grow the pattern by one more bar so there's always room ahead —
  // lets the user keep drawing notes indefinitely instead of hitting a wall.
  $effect(() => {
    const bar = channelStore.barLength;
    const lastBarStart = channelStore.patternLength - bar;
    for (const ch of channelStore.channels) {
      for (const note of ch.notes) {
        if (note.start + note.length > lastBarStart) {
          channelStore.patternLength += bar;
          return;
        }
      }
    }
  });

  function close() { show = false; }

  function handleScroll(left: number, top: number) {
    gridScrollLeft = left;
    gridScrollTop = top;
  }

  function handleSeek(x: number) {
    noteGridRef?.scrollTo(x, gridScrollTop);
  }

  // Playing from the Piano Roll's own button solos just this channel — the
  // shared clock/engine still only ever runs once (see playbackStore).
  function handleTogglePlay() {
    if (playback.isPlaying) {
      playback.isPlaying = false;
    } else if (channel) {
      // Soloing a single channel only makes sense against pattern-mode's
      // flat channel list — song mode's scheduler has no concept of it, so
      // force pattern mode first (mirrors ChannelRack's own play button).
      if (playback.transportMode === 'song') playback.transportMode = 'pattern';
      playback.soloChannelId = channel.id;
      playback.isPlaying = true;
    }
  }
</script>

{#if channel}
  <FloatingWindow
    id="piano-roll"
    bind:show
    {workspaceBounds}
    x={220} y={140} width={760} height={480}
    minWidth={400} minHeight={260}
  >
    {#snippet header({ onDragStart, maximized, toggleMaximize })}
      <PianoRollHeader
        bind:tool
        bind:showNoteProps
        title={channelName}
        {maximized}
        isPlaying={playback.isPlaying}
        onClose={close}
        onDragStart={onDragStart}
        onToggleMaximize={toggleMaximize}
        onTogglePlay={handleTogglePlay}
      />
    {/snippet}

    <div class="pr-shell">
      {#if showPianoRollOverview}
        <div class="pr-overview-slot">
          <PianoRollOverview
            {channel}
            patternLength={channelStore.patternLength}
            activeStep={wrappedStep}
            scrollLeft={gridScrollLeft}
            viewportWidth={gridViewportWidth}
            onSeek={handleSeek}
          />
        </div>
      {/if}
      <div class="pr-main">
      {#if showNoteProps}
        <NotePropertiesPanel {channel} {selectedNoteIds} />
      {/if}
      <div class="pr-body" style="--key-col-w:{KEY_COL_W}px; --ruler-h:{RULER_H}px; --lane-h:{LANE_H}px;">
        <div class="pr-corner"></div>
        <div class="pr-ruler-slot">
          <PianoRollRuler patternLength={channelStore.patternLength} scrollLeft={gridScrollLeft} activeStep={wrappedStep} />
        </div>
        <div class="pr-keys-slot">
          <PianoKeys scrollTop={gridScrollTop} {highlightPitches} midiPitches={liveNotes.held} />
        </div>
        <div class="pr-grid-slot">
          <NoteGrid
            bind:this={noteGridRef}
            {channel}
            {tool}
            bind:selectedNoteIds
            patternLength={channelStore.patternLength}
            activeStep={wrappedStep}
            currentStepFraction={wrappedStepFraction}
            onScroll={handleScroll}
            onViewportResize={(w) => gridViewportWidth = w}
            onDragPitchesChange={(p) => highlightPitches = p}
            midiPitches={liveNotes.held}
          />
        </div>
        <div class="pr-vel-corner"></div>
        <div class="pr-vel-slot">
          <VelocityLane {channel} {selectedNoteIds} scrollLeft={gridScrollLeft} patternLength={channelStore.patternLength} />
        </div>
      </div>
      </div>
    </div>
  </FloatingWindow>
{/if}

<style>
  .pr-shell {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }

  .pr-overview-slot {
    flex-shrink: 0;
    height: 40px;
    padding: 4px 8px;
    background: #1d1f29;
    border-bottom: 1px solid var(--explorer-border, #383c4a);
  }

  .pr-main {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .pr-body {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-columns: var(--key-col-w) 1fr;
    grid-template-rows: var(--ruler-h) 1fr var(--lane-h);
  }

  .pr-corner,
  .pr-vel-corner {
    background: #1d1f29;
    border-right: 1px solid var(--explorer-border, #383c4a);
    border-bottom: 1px solid var(--explorer-border, #383c4a);
  }

  .pr-ruler-slot { min-width: 0; }
  .pr-keys-slot  { min-height: 0; border-right: 1px solid var(--explorer-border, #383c4a); }
  .pr-grid-slot  { min-width: 0; min-height: 0; }
  .pr-vel-slot   { min-width: 0; border-top: 1px solid var(--explorer-border, #383c4a); }
</style>
