<script lang="ts">
  import { applyDropToChannel } from '$lib/channelDrop';
  import Dial from './Dial.svelte';
  import ScrollField from './ScrollField.svelte';
  import ChannelRow from './ChannelRow.svelte';
  import FloatingWindow, { type WorkspaceBounds } from './FloatingWindow.svelte';
  import { playback } from '$lib/playbackStore.svelte';
  import { channelStore } from '$lib/channelStore.svelte';
  import { portal } from '$lib/portal';
  import type { FileDropDetail, FileDragOverDetail } from '$lib/types';

  let {
    show = $bindable(),
    workspaceBounds,
    showPianoRollOverview = $bindable(false),
    onOpenPianoRoll,
  }: {
    show: boolean;
    workspaceBounds: WorkspaceBounds;
    showPianoRollOverview?: boolean;
    onOpenPianoRoll: (channelId: number) => void;
  } = $props();

  // ── Name column resize ─────────────────────────────────────────────
  let nameColWidth = $state(140);
  let colResizing = false;
  let resizeStartX = 0;
  let resizeStartW = 0;

  function onResizeMousedown(e: MouseEvent) {
    e.stopPropagation();
    e.preventDefault();
    colResizing = true;
    resizeStartX = e.clientX;
    resizeStartW = nameColWidth;
  }

  function onWindowMousemove(e: MouseEvent) {
    if (colResizing) {
      nameColWidth = Math.max(80, Math.min(280, resizeStartW + (e.clientX - resizeStartX)));
    }
  }

  function onWindowMouseup() {
    colResizing = false;
  }

  // ── Header toggles ─────────────────────────────────────────────────
  let loopStart   = $state(false);
  let loopMode    = $state(false);
  let graphEditor = $state(false);

  // ── Swing ─────────────────────────────────────────────────────────
  let swing = $state(0);

  // ── Dropdowns ──────────────────────────────────────────────────────
  let optionsOpen = $state(false);
  let filterOpen  = $state(false);
  let filter      = $state<'all' | 'unsorted'>('all');

  function toggleOptions(e: MouseEvent) {
    e.stopPropagation();
    optionsOpen = !optionsOpen;
    if (optionsOpen) filterOpen = false;
  }

  function toggleFilter(e: MouseEvent) {
    e.stopPropagation();
    filterOpen = !filterOpen;
    if (filterOpen) optionsOpen = false;
  }

  function closeDropdowns() {
    optionsOpen = false;
    filterOpen = false;
  }

  // ── Selection ──────────────────────────────────────────────────────
  let selectedIds     = $state(new Set<number>());
  let lastSelectedIdx = -1;

  function handleSelect(i: number, e: MouseEvent) {
    const id = channelStore.channels[i].id;
    if (e.ctrlKey || e.metaKey) {
      const next = new Set(selectedIds);
      if (next.has(id)) next.delete(id); else next.add(id);
      selectedIds = next;
    } else if (e.shiftKey && lastSelectedIdx >= 0) {
      const lo = Math.min(lastSelectedIdx, i);
      const hi = Math.max(lastSelectedIdx, i);
      selectedIds = new Set(channelStore.channels.slice(lo, hi + 1).map(c => c.id));
    } else {
      selectedIds = new Set([id]);
    }
    lastSelectedIdx = i;
  }

  function handleDelete(id: number) {
    channelStore.removeChannel(id);
    if (selectedIds.has(id)) {
      const next = new Set(selectedIds);
      next.delete(id);
      selectedIds = next;
    }
  }

  function close() { show = false; }

  // ── Drop a sample/folder straight into the rack to create a new channel ──
  // Dropping onto an existing channel's own sample-name box still just
  // assigns that sample to it (ChannelRack's `onSampleDrop` above, via
  // ChannelRow's own `data-sample-drop` div) — this is the separate case of
  // dropping anywhere else in the list, which inserts a brand-new channel at
  // the dropped position instead. `rack-list` is itself a `data-sample-drop`
  // target, and `elementsFromPoint` (FileExplorer.svelte) always finds the
  // innermost matching element first, so a row's own sample box still wins
  // when the cursor is directly over it.
  let rackListEl = $state<HTMLElement | null>(null);
  let dropInsertIndex = $state<number | null>(null);

  function computeInsertIndex(clientY: number): number {
    if (!rackListEl) return channelStore.channels.length;
    const rows = Array.from(rackListEl.querySelectorAll(':scope > .ch-row')) as HTMLElement[];
    for (let i = 0; i < rows.length; i++) {
      const rect = rows[i].getBoundingClientRect();
      if (clientY < rect.top + rect.height / 2) return i;
    }
    return rows.length;
  }

  // Local (scroll-aware) top offset of the boundary at `index`, for
  // positioning the insertion-line indicator absolutely inside `.rack-list`.
  function insertLineTop(index: number): number {
    if (!rackListEl) return 0;
    const rows = Array.from(rackListEl.querySelectorAll(':scope > .ch-row')) as HTMLElement[];
    const listRect = rackListEl.getBoundingClientRect();
    let clientTop: number;
    if (rows.length === 0) {
      clientTop = listRect.top;
    } else if (index >= rows.length) {
      clientTop = rows[rows.length - 1].getBoundingClientRect().bottom;
    } else {
      clientTop = rows[index].getBoundingClientRect().top;
    }
    return clientTop - listRect.top + rackListEl.scrollTop;
  }

  function insertChannelFromDrop(drop: FileDropDetail, index: number) {
    const ch = channelStore.addChannel(index);
    applyDropToChannel(ch, drop);
  }

  $effect(() => {
    if (!rackListEl) return;
    const el = rackListEl;
    function onDragOver(e: Event) {
      dropInsertIndex = computeInsertIndex((e as CustomEvent<FileDragOverDetail>).detail.y);
    }
    function onLeave() { dropInsertIndex = null; }
    function onDrop(e: Event) {
      const index = dropInsertIndex ?? channelStore.channels.length;
      dropInsertIndex = null;
      insertChannelFromDrop((e as CustomEvent<FileDropDetail>).detail, index);
    }
    el.addEventListener('filedragover', onDragOver);
    el.addEventListener('filedragleave', onLeave);
    el.addEventListener('filedrop', onDrop);
    return () => {
      el.removeEventListener('filedragover', onDragOver);
      el.removeEventListener('filedragleave', onLeave);
      el.removeEventListener('filedrop', onDrop);
    };
  });

  // ── Growing the step sequencer ───────────────────────────────────────
  // Two ways to add steps to every channel (channelStore.addSteps, shared
  // globally across channels/patterns — see channelStore.svelte.ts):

  // 1. Drag the window wider: whenever the rack list grows enough trailing
  // blank space (past the last step button of a plain step-sequencer row)
  // to fit a whole extra beat's worth of steps (4 × 26px, matching .step's
  // 24px width + .steps' 2px gap), add that many beats. Measured live off
  // the DOM (same convention as computeInsertIndex/insertLineTop above)
  // rather than hand-tracking every fixed control's width, so it stays
  // correct if row layout ever changes. Only ever grows — shrinking the
  // window never removes steps, since addSteps() itself is additive-only.
  let rackListWidth = $state(0);

  $effect(() => {
    // Reactive deps: rack width and the name column (both affect how much
    // room steps have). Deliberately *not* channelStore.stepCount — a
    // single run already computes however many beats currently fit in one
    // shot (beatsFit below isn't a "check again" loop, it's the whole
    // gap's worth), so re-triggering on stepCount isn't needed for growing
    // to work, and it would actively fight "Trim steps" (the rack's own
    // context menu, below): trimming shrinks stepCount, which would
    // otherwise immediately re-run this effect and re-add exactly what was
    // just trimmed if the window still had room. Leaving stepCount out
    // means trimmed steps stay trimmed until the window is actually
    // dragged wider again.
    void rackListWidth;
    void nameColWidth;
    if (!rackListEl) return;
    // A note-based channel's `.steps` div renders a MiniNoteRoll instead of
    // step buttons, sized completely differently — measure the first row
    // that's still a plain step sequencer, and do nothing if there isn't
    // one (nothing reliable to measure against).
    const idx = channelStore.channels.findIndex(c => c.notes.length === 0);
    if (idx === -1) return;
    const rows = rackListEl.querySelectorAll(':scope > .ch-row');
    const row = rows[idx] as HTMLElement | undefined;
    const stepsEl = row?.querySelector(':scope > .steps') as HTMLElement | null | undefined;
    if (!row || !stepsEl) return;
    const rowRect = row.getBoundingClientRect();
    const stepsRect = stepsEl.getBoundingClientRect();
    const ROW_PAD_RIGHT = 8; // .ch-row's `padding: 6px 8px`
    const STEP_CELL = 26;    // .step width (24px) + .steps gap (2px)
    const trailingGap = rowRect.right - ROW_PAD_RIGHT - stepsRect.right;
    const beatsFit = Math.floor(trailingGap / (STEP_CELL * 4));
    if (beatsFit >= 1) channelStore.addSteps(beatsFit);
  });

  // 2. Right-click empty space in the rack (not on a row, which has its own
  // fill/rename/recolor/delete menu — ChannelRow.svelte) → "Add steps…" →
  // type a number of beats (groups of 4 steps) → Enter commits. Same
  // swap-the-menu-body-for-an-input convention as ChannelRow's own Rename.
  let rackContextMenu = $state<{ x: number; y: number } | null>(null);
  let rackMenuView = $state<'main' | 'addSteps'>('main');
  let addStepsDraft = $state(4);
  let addStepsInputEl = $state<HTMLInputElement | null>(null);

  function openRackContextMenu(e: MouseEvent) {
    if ((e.target as HTMLElement).closest('.ch-row')) return; // rows own their own menu
    e.preventDefault();
    rackContextMenu = { x: e.clientX, y: e.clientY };
    rackMenuView = 'main';
  }

  function closeRackContextMenu() {
    rackContextMenu = null;
    rackMenuView = 'main';
  }

  function openAddSteps() {
    addStepsDraft = 4;
    rackMenuView = 'addSteps';
  }

  $effect(() => {
    if (rackMenuView === 'addSteps') addStepsInputEl?.select();
  });

  function commitAddSteps() {
    channelStore.addSteps(Math.max(1, Math.round(addStepsDraft || 0)));
    closeRackContextMenu();
  }

  function addStepsKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') { e.preventDefault(); commitAddSteps(); }
    else if (e.key === 'Escape') { e.preventDefault(); closeRackContextMenu(); }
  }

  // "Trim steps" — the inverse of "Add steps…": drops however many trailing
  // steps are unused by every channel in every pattern (channelStore.trimSteps,
  // see its own comment for the "only the end, never the middle" rule). A
  // single-click action, same as ChannelRow's own "Recolor randomly"/"Delete
  // channel" menu items, no sub-view needed.
  function trimSteps() {
    channelStore.trimSteps();
    closeRackContextMenu();
  }
</script>

<svelte:window onmousemove={onWindowMousemove} onmouseup={onWindowMouseup} />

{#if show && (optionsOpen || filterOpen)}
  <!-- Backdrop to close dropdowns when clicking outside them -->
  <div class="dd-backdrop" onmousedown={closeDropdowns} role="presentation"></div>
{/if}

<FloatingWindow
  id="channel-rack"
  bind:show
  {workspaceBounds}
  x={120} y={80} width={720} height={420}
  minWidth={480} minHeight={220}
>
  {#snippet header({ onDragStart, maximized, toggleMaximize })}
    <!-- ══ TITLE BAR ══════════════════════════════════════════════════ -->
    <div class="rack-header" onmousedown={onDragStart} role="toolbar" aria-label="Channel Rack controls" tabindex="-1">

      <!-- Left group -->
      <div class="hdr-left">
        <!-- 1. Channel options -->
        <div class="dd-wrap">
          <button class="hdr-btn" onclick={toggleOptions} aria-label="Channel options" title="Channel options">
            <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round">
              <circle cx="7" cy="7" r="2.2"/>
              <path d="M7 1v1.5M7 11.5V13M1 7h1.5M11.5 7H13M2.9 2.9l1.1 1.1M10 10l1.1 1.1M11.1 2.9L10 4M4 10l-1.1 1.1"/>
            </svg>
          </button>
          {#if optionsOpen}
            <div class="dropdown" role="menu">
              <button class="dd-item" role="menuitem" onclick={closeDropdowns}>Channel settings</button>
              <button class="dd-item" role="menuitem" onclick={closeDropdowns}>Rename</button>
              <button class="dd-item" role="menuitem" onclick={closeDropdowns}>Color</button>
            </div>
          {/if}
        </div>

        <!-- 2. Loop start toggle -->
        <button
          class="hdr-btn hdr-tgl"
          class:hdr-tgl--on={loopStart}
          onclick={(e) => { e.stopPropagation(); loopStart = !loopStart; }}
          aria-label="Loop start mode"
          title="Loop start mode"
        >
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
            <path d="M2 7a5 5 0 1 0 5-5"/>
            <polyline points="7,1 7,3.5 4.5,2.2"/>
          </svg>
        </button>

        <!-- 3. Filter dropdown -->
        <div class="dd-wrap">
          <button class="hdr-btn hdr-filter" onclick={toggleFilter} aria-label="Filter" title="Filter">
            <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round">
              <path d="M1 2h12L9 7v5H5V7L1 2z"/>
            </svg>
            <span class="filter-label">{filter === 'all' ? 'ALL' : 'UNSORTED'}</span>
          </button>
          {#if filterOpen}
            <div class="dropdown" role="menu">
              <button class="dd-item" class:dd-item--active={filter === 'all'} role="menuitem"
                onclick={() => { filter = 'all'; filterOpen = false; }}>All</button>
              <button class="dd-item" class:dd-item--active={filter === 'unsorted'} role="menuitem"
                onclick={() => { filter = 'unsorted'; filterOpen = false; }}>Unsorted</button>
            </div>
          {/if}
        </div>

        <!-- 4. Resize handle -->
        <div
          class="resize-handle"
          onmousedown={onResizeMousedown}
          role="slider"
          aria-label="Resize name column"
          title="Drag to resize sample name column"
          aria-valuenow={nameColWidth}
          aria-valuemin={80}
          aria-valuemax={280}
          tabindex="0"
        >
          <svg viewBox="0 0 8 14" fill="currentColor">
            <circle cx="4" cy="3"  r="1.5"/>
            <circle cx="4" cy="7"  r="1.5"/>
            <circle cx="4" cy="11" r="1.5"/>
          </svg>
        </div>

        <!-- 5. Play/stop toggle -->
        <button
          class="hdr-btn hdr-tgl"
          class:hdr-tgl--on={playback.isPlaying}
          onclick={(e) => {
            e.stopPropagation();
            if (!playback.isPlaying) {
              playback.soloChannelId = null;
              if (playback.transportMode === 'song') playback.transportMode = 'pattern';
            }
            playback.isPlaying = !playback.isPlaying;
          }}
          aria-label={playback.isPlaying ? 'Stop' : 'Play'}
          title={playback.isPlaying ? 'Stop' : 'Play'}
        >
          {#if playback.isPlaying}
            <svg viewBox="0 0 14 14" fill="currentColor">
              <rect x="3" y="3" width="3" height="8" rx="0.5"/>
              <rect x="8" y="3" width="3" height="8" rx="0.5"/>
            </svg>
          {:else}
            <svg viewBox="0 0 14 14" fill="currentColor">
              <path d="M4 2.5l7 4.5-7 4.5z"/>
            </svg>
          {/if}
        </button>
      </div>

      <!-- Center title -->
      <span class="rack-title">Channel Rack</span>

      <!-- Right group -->
      <div class="hdr-right">
        <!-- 7. Swing dial -->
        <div class="dial-group">
          <Dial bind:value={swing} defaultValue={0} showArc={true} size={26} sensitivity={180} tooltipKey="SWING" name="Swing dial" />
          <span class="dial-label">SWING</span>
        </div>

        <!-- 8. Time signature (beats per bar; denominator fixed at quarter notes) -->
        <div class="sf-group">
          <span class="ts-wrap">
            <ScrollField bind:value={channelStore.beatsPerBar} min={1} max={16} width={16} tooltipKey="TIME SIG" name="Time signature" />
            <span class="ts-slash">/4</span>
          </span>
          <span class="dial-label">TIME SIG</span>
        </div>

        <!-- 8b. Pattern length (always a whole multiple of one bar) -->
        <div class="sf-group">
          <ScrollField
            bind:value={channelStore.patternLength}
            min={channelStore.barLength}
            max={channelStore.barLength * 64}
            step={channelStore.barLength}
            width={34}
            tooltipKey="LEN"
            name="Pattern length"
          />
          <span class="dial-label">LEN</span>
        </div>

        <!-- 9. Loop mode -->
        <button
          class="hdr-btn hdr-tgl"
          class:hdr-tgl--on={loopMode}
          onclick={(e) => { e.stopPropagation(); loopMode = !loopMode; }}
          aria-label="Loop mode"
          title="Loop mode"
        >
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 7a5 5 0 1 0-5 5"/>
            <polyline points="7,13 7,10.5 9.5,11.8"/>
          </svg>
        </button>

        <!-- 10. Graph editor -->
        <button
          class="hdr-btn hdr-tgl"
          class:hdr-tgl--on={graphEditor}
          onclick={(e) => { e.stopPropagation(); graphEditor = !graphEditor; }}
          aria-label="Graph editor"
          title="Graph editor"
        >
          <svg viewBox="0 0 14 14" fill="currentColor">
            <rect x="1"   y="6" width="3" height="7" rx="0.5"/>
            <rect x="5.5" y="3" width="3" height="10" rx="0.5"/>
            <rect x="10"  y="5" width="3" height="8" rx="0.5"/>
          </svg>
        </button>

        <!-- 11. Piano roll overview -->
        <button
          class="hdr-btn hdr-tgl"
          class:hdr-tgl--on={showPianoRollOverview}
          onclick={(e) => { e.stopPropagation(); showPianoRollOverview = !showPianoRollOverview; }}
          aria-label="Piano roll overview"
          title="Piano roll overview"
        >
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1">
            <rect x="1" y="1" width="12" height="12" rx="1"/>
            <line x1="1"  y1="5"  x2="13" y2="5"/>
            <line x1="1"  y1="9"  x2="13" y2="9"/>
            <line x1="5"  y1="1"  x2="5"  y2="13"/>
            <line x1="9"  y1="1"  x2="9"  y2="13"/>
            <rect x="5" y="1" width="4" height="4" fill="currentColor" opacity="0.45" stroke="none"/>
          </svg>
        </button>

        <!-- 12. Maximize / restore -->
        <button
          class="hdr-btn"
          onclick={(e) => { e.stopPropagation(); toggleMaximize(); }}
          aria-label={maximized ? 'Restore' : 'Maximize'}
          title={maximized ? 'Restore' : 'Maximize'}
        >
          {#if maximized}
            <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
              <rect x="4" y="4" width="8" height="8" rx="1"/>
              <path d="M2 8V3a1 1 0 0 1 1-1h5"/>
            </svg>
          {:else}
            <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
              <rect x="2" y="2" width="10" height="10" rx="1"/>
            </svg>
          {/if}
        </button>

        <!-- 13. Close -->
        <button class="close-btn" onclick={close} aria-label="Close">
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <line x1="3" y1="3" x2="11" y2="11"/>
            <line x1="11" y1="3" x2="3" y2="11"/>
          </svg>
        </button>
      </div>
    </div>
  {/snippet}

  <!-- ══ CHANNEL LIST ═══════════════════════════════════════════════ -->
  <div
    class="rack-list"
    id="rack-list"
    data-sample-drop
    bind:this={rackListEl}
    bind:clientWidth={rackListWidth}
    oncontextmenu={openRackContextMenu}
    role="group"
    aria-label="Channel list"
  >
    {#if dropInsertIndex !== null}
      <div class="insert-line" style="top:{insertLineTop(dropInsertIndex)}px;"></div>
    {/if}
    {#each channelStore.channels as ch, i (ch.id)}
      <ChannelRow
        channel={ch}
        {nameColWidth}
        selected={selectedIds.has(ch.id)}
        activeStep={playback.currentStep}
        onSelect={(e) => handleSelect(i, e)}
        onStepChange={(step, active) => { channelStore.channels[i].steps[step] = active; }}
        onSampleDrop={(drop) => applyDropToChannel(channelStore.channels[i], drop)}
        onOpenPianoRoll={() => onOpenPianoRoll(ch.id)}
        onDelete={() => handleDelete(ch.id)}
      />
    {/each}
  </div>

  <!-- ══ BOTTOM BAR ═════════════════════════════════════════════════ -->
  <div class="rack-footer">
    <button class="add-btn" onclick={() => channelStore.addChannel()} aria-label="Add channel" title="Add channel">+</button>
    <div class="h-scroll-track" role="scrollbar" aria-controls="rack-list" aria-orientation="horizontal" aria-valuenow={0} aria-valuemin={0} aria-valuemax={100}>
      <div class="h-scroll-thumb"></div>
    </div>
  </div>
</FloatingWindow>

{#if rackContextMenu}
  <div class="ctx-root" use:portal>
    <div class="ctx-backdrop" onmousedown={closeRackContextMenu} oncontextmenu={(e) => e.preventDefault()} role="presentation"></div>
    <div class="ctx-menu" style="left:{rackContextMenu.x}px; top:{rackContextMenu.y}px;" role="menu">
      {#if rackMenuView === 'main'}
        <button class="ctx-item" role="menuitem" onclick={openAddSteps}>Add steps…</button>
        <div class="ctx-sep"></div>
        <button class="ctx-item" role="menuitem" onclick={trimSteps}>Trim steps</button>
      {:else}
        <input
          class="ctx-input"
          type="number"
          min="1"
          step="1"
          bind:value={addStepsDraft}
          bind:this={addStepsInputEl}
          onkeydown={addStepsKeydown}
          placeholder="Beats"
        />
      {/if}
    </div>
  </div>
{/if}

<style>
  /* ── Backdrop ──────────────────────────────────────────────────── */
  .dd-backdrop {
    position: fixed;
    inset: 0;
    z-index: 150;
  }

  /* ── Header ────────────────────────────────────────────────────── */
  .rack-header {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px;
    height: 40px;
    background: #1d1f29;
    border-bottom: 1px solid var(--explorer-border, #383c4a);
    flex-shrink: 0;
    user-select: none;
  }

  .hdr-left,
  .hdr-right {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }

  .rack-title {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    font-family: 'Playfair Display', Georgia, serif;
    font-size: 14px;
    font-weight: 600;
    color: var(--main-text, #bcbcc0);
    pointer-events: none;
    white-space: nowrap;
  }

  /* ── Header buttons ────────────────────────────────────────────── */
  .hdr-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    color: rgba(255,255,255,0.45);
    cursor: pointer;
    padding: 0;
    transition: color 0.1s, background 0.1s, border-color 0.1s;
  }

  .hdr-btn svg { width: 14px; height: 14px; }

  .hdr-btn:hover {
    background: rgba(255,255,255,0.07);
    color: rgba(255,255,255,0.75);
  }

  .hdr-tgl--on {
    color: var(--accent, #e6b07f);
    border-color: rgba(230,176,127,0.3);
  }

  .hdr-tgl--on:hover {
    color: var(--accent, #e6b07f);
    background: rgba(230,176,127,0.1);
  }

  /* Filter button (wider due to label) */
  .hdr-filter {
    width: auto;
    gap: 3px;
    padding: 0 5px;
  }

  .filter-label {
    font-size: 9px;
    letter-spacing: 0.06em;
    font-family: 'DM Mono', monospace;
    line-height: 1;
  }

  /* Resize handle */
  .resize-handle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 26px;
    cursor: col-resize;
    color: rgba(255,255,255,0.3);
    border-radius: 5px;
    transition: color 0.1s, background 0.1s;
    flex-shrink: 0;
  }

  .resize-handle:hover {
    color: rgba(255,255,255,0.65);
    background: rgba(255,255,255,0.07);
  }

  .resize-handle svg { width: 8px; height: 14px; }

  /* Dial + label group */
  .dial-group,
  .sf-group {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
  }

  .dial-label {
    font-size: 7px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: rgba(255,255,255,0.3);
    font-family: 'DM Mono', monospace;
    line-height: 1;
    pointer-events: none;
  }

  .ts-wrap {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .ts-slash {
    font-family: 'DM Mono', monospace;
    font-size: 10px;
    color: rgba(255,255,255,0.35);
  }

  /* Close button */
  .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    color: rgba(255,255,255,0.4);
    cursor: pointer;
    padding: 0;
    transition: color 0.1s, background 0.1s;
    margin-left: 2px;
  }

  .close-btn svg { width: 12px; height: 12px; }

  .close-btn:hover {
    background: rgba(220,50,50,0.18);
    color: #f08484;
    border-color: rgba(220,50,50,0.3);
  }

  /* ── Dropdowns ─────────────────────────────────────────────────── */
  .dd-wrap {
    position: relative;
  }

  .dropdown {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 151;
    background: #272a33;
    border: 1px solid var(--explorer-border, #34373f);
    border-radius: 8px;
    box-shadow: 0 6px 18px rgba(0,0,0,0.5);
    min-width: 130px;
    padding: 3px;
    display: flex;
    flex-direction: column;
  }

  .dd-item {
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

  .dd-item:hover {
    background: rgba(255,255,255,0.07);
    color: var(--main-text, #bcbcc0);
  }

  .dd-item--active {
    color: var(--accent, #e6b07f);
  }

  /* ── Channel list ──────────────────────────────────────────────── */
  .rack-list {
    position: relative; /* anchors .insert-line's absolute positioning */
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    min-height: 60px;
  }

  /* Drop-to-insert position indicator — see ChannelRack's rackListEl drag
     handling above. Sits between rows (or above the first/below the last),
     never inside one, so "drop as first/last/anywhere in between" reads
     visually as a precise insertion point rather than a row highlight. */
  .insert-line {
    position: absolute;
    left: 8px;
    right: 8px;
    height: 2px;
    margin-top: -1px; /* center the line on the computed boundary */
    background: var(--accent, #e6b07f);
    border-radius: 2px;
    box-shadow: 0 0 6px rgba(230,176,127,0.7);
    pointer-events: none;
    z-index: 5;
  }

  .rack-list::-webkit-scrollbar { width: 6px; }
  .rack-list::-webkit-scrollbar-track { background: transparent; }
  .rack-list::-webkit-scrollbar-thumb {
    background: var(--explorer-border, #34373f);
    border-radius: 5px;
  }

  /* ── Footer ────────────────────────────────────────────────────── */
  .rack-footer {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    background: #1d1f29;
    border-top: 1px solid var(--explorer-border, #383c4a);
    flex-shrink: 0;
    height: 28px;
  }

  .add-btn {
    width: 20px;
    height: 20px;
    border-radius: 5px;
    border: 1px solid rgba(255,255,255,0.15);
    background: transparent;
    color: rgba(255,255,255,0.6);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
    flex-shrink: 0;
  }

  .add-btn:hover {
    background: rgba(255,255,255,0.08);
    color: var(--main-text, #bcbcc0);
    border-color: rgba(255,255,255,0.3);
  }

  .h-scroll-track {
    flex: 1;
    height: 8px;
    background: rgba(255,255,255,0.05);
    border-radius: 7px;
    border: 1px solid rgba(255,255,255,0.07);
    overflow: hidden;
  }

  .h-scroll-thumb {
    width: 40%;
    height: 100%;
    background: rgba(255,255,255,0.18);
    border-radius: 7px;
  }

  /* ── Rack-level context menu ("Add steps…") ───────────────────────
     Same shape/styling as ChannelRow.svelte's own row context menu. */
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

  .ctx-sep {
    height: 1px;
    margin: 4px 6px;
    background: rgba(255,255,255,0.08);
  }

  .ctx-input {
    width: 100px;
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
</style>
