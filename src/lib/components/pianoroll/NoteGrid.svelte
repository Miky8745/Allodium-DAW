<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type { ChannelData, Note } from '$lib/types';
  import type { PianoRollTool } from './PianoRollHeader.svelte';
  import {
    MIN_PITCH, MAX_PITCH, DEFAULT_CENTER_PITCH,
    keyH, stepW, gridTotalH,
    patternWidth, stepToX, pitchToY, xToStep, xToStepFree, yToPitch, clampPitch, isBlackKey, isCNote, pitchName,
  } from '$lib/pianoroll/pitch';
  import { audioEngine } from '$lib/audioEngine';
  import { pianoRollZoom } from '$lib/pianoRollZoom.svelte';
  import { noteDefaults } from '$lib/noteDefaultsStore.svelte';
  import { noteColorStore } from '$lib/noteColorStore.svelte';
  import { readableTextOn } from '$lib/colorUtils';
  import { windowFocus } from '$lib/windowFocusStore.svelte';
  import { markDeleted, deleteOut } from '$lib/deleteFx';
  import { beginDragCursor, endDragCursor } from '$lib/dragCursor';

  let {
    channel,
    tool,
    selectedNoteIds = $bindable(),
    patternLength,
    activeStep = -1,
    currentStepFraction = -1,
    onScroll,
    onViewportResize,
    onDragPitchesChange,
    midiPitches,
  }: {
    channel: ChannelData;
    tool: PianoRollTool;
    selectedNoteIds: Set<number>;
    patternLength: number;
    activeStep?: number;
    currentStepFraction?: number;
    onScroll: (scrollLeft: number, scrollTop: number) => void;
    onViewportResize?: (width: number) => void;
    onDragPitchesChange?: (pitches: Set<number>) => void;
    // Live-held MIDI/QWERTY notes (pitch -> 0..1 velocity), drawn as a
    // faint glow across the whole matching row.
    midiPitches?: ReadonlyMap<number, number>;
  } = $props();

  const pitches = Array.from({ length: MAX_PITCH - MIN_PITCH + 1 }, (_, i) => MAX_PITCH - i);

  let viewportEl = $state<HTMLElement | null>(null);
  let contentEl  = $state<HTMLElement | null>(null);

  let nextNoteId = 1;
  $effect(() => {
    for (const n of channel.notes) if (n.id >= nextNoteId) nextNoteId = n.id + 1;
  });

  const DEFAULT_NOTE_LENGTH = 8; // 2 beats × 4 steps/beat — default length before any note has been clicked
  const MIN_FREE_LENGTH = 0.05;  // steps — floor for Shift-free resize so length never hits 0/negative

  // Remembers the last note created/clicked/moved/resized, so the next
  // newly-placed note reuses its length. Plain bookkeeping (same category as
  // nextNoteId/clipboard below) — never read from a template, so it doesn't
  // need to be $state.
  let lastLength = DEFAULT_NOTE_LENGTH;

  function localPos(e: MouseEvent | PointerEvent): { x: number; y: number } {
    const rect = contentEl!.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  }

  function noteAt(step: number, pitch: number): Note | undefined {
    return channel.notes.find(n => n.pitch === pitch && step >= n.start && step < n.start + n.length);
  }

  function createNote(step: number, pitch: number, length: number): Note {
    // New notes take the properties panel's current defaults (color, slide,
    // velocity, pan, …) — see noteDefaultsStore.svelte.ts.
    const note: Note = { ...noteDefaults.props, id: nextNoteId++, pitch, start: step, length };
    channel.notes.push(note);
    return note;
  }

  // Key for the deletion fade + red ring (deleteFx.ts) — only notes marked
  // with it animate out; pattern switches/undo just make notes vanish.
  const delKey = (id: number) => `note:${channel.id}:${id}`;

  function deleteNote(note: Note) {
    markDeleted(delKey(note.id));
    const idx = channel.notes.indexOf(note);
    if (idx !== -1) channel.notes.splice(idx, 1);
    if (selectedNoteIds.has(note.id)) {
      const next = new Set(selectedNoteIds);
      next.delete(note.id);
      selectedNoteIds = next;
    }
  }

  // ── Draw tool ─────────────────────────────────────────────────────
  // Click on empty space places one note of lastLength (in the current
  // default color); keeping the button down and dragging moves that new
  // note (position + pitch, like dragging an existing one — length only
  // changes by grabbing a note's right edge). Clicking a note selects it
  // (Shift adds — only needed for multi-select), adopts its color/length for
  // the next placed note, and dragging moves the whole selection; grabbing
  // a note's last ~6px resizes it instead.
  type DrawDrag = { mode: 'resize-existing'; noteId: number; anchorStepSnapped: number; anchorStepFree: number; origLength: number };

  // $state (not a plain let) so the highlightPitches derivation below can
  // react to the live drag.
  let drawDrag = $state<DrawDrag | null>(null);

  // Clicking a note makes it the template for the next placed one.
  function adoptTemplate(note: Note) {
    lastLength = note.length;
    noteDefaults.props.color = note.color;
  }

  function drawMousedown(e: MouseEvent) {
    const { x, y } = localPos(e);
    const stepSnapped = xToStep(x);
    const stepFree = xToStepFree(x);
    const pitch = yToPitch(y);
    if (stepSnapped >= patternLength) return;

    const hit = noteAt(stepSnapped, pitch);
    if (!hit) {
      // Shift+drag on empty space marquee-selects (adding) from Draw.
      if (e.shiftKey) { startMarquee(e, x, y); return; }
      clearSelection(); // clicking empty space deselects (and draws)
      const len = Math.max(1, Math.min(patternLength - stepSnapped, lastLength));
      const note = createNote(stepSnapped, pitch, len);
      // Drag the just-placed note by itself (it isn't selected).
      startMoveDrag(note, x, stepSnapped, pitch, false, [note.id]);
      return;
    }

    adoptTemplate(hit);
    if (e.shiftKey) {
      selectedNoteIds = new Set([...selectedNoteIds, hit.id]);
    } else if (!selectedNoteIds.has(hit.id)) {
      selectedNoteIds = new Set([hit.id]);
    }
    // Plain click on a note already in a multi-selection: keep the group for
    // a drag, but a click without moving narrows the selection to it.
    const collapseOnClick = !e.shiftKey && selectedNoteIds.size > 1;

    if (!e.shiftKey && x >= stepToX(hit.start + hit.length) - 6) {
      audioEngine.previewNote(channel, hit.pitch, hit);
      beginDragCursor('col-resize');
      drawDrag = {
        mode: 'resize-existing', noteId: hit.id,
        anchorStepSnapped: stepSnapped, anchorStepFree: stepFree, origLength: hit.length,
      };
      return;
    }
    startMoveDrag(hit, x, stepSnapped, pitch, collapseOnClick);
  }

  function drawPointermove(e: PointerEvent) {
    if (!drawDrag) return;
    if (e.buttons === 0) { commitDrawDrag(); return; }
    const { x } = localPos(e);
    const free = e.shiftKey;
    const current = free ? xToStepFree(x) : xToStep(x);
    const minLen = free ? MIN_FREE_LENGTH : 1;
    const note = channel.notes.find(n => n.id === drawDrag!.noteId);
    if (!note) return;
    const anchor = free ? drawDrag.anchorStepFree : drawDrag.anchorStepSnapped;
    note.length = Math.max(minLen, Math.min(patternLength - note.start, drawDrag.origLength + (current - anchor)));
    // pitch/y intentionally never read here — length-only, row locked to
    // the pitch the note was clicked on.
  }

  function commitDrawDrag() {
    if (drawDrag) {
      const note = channel.notes.find(n => n.id === drawDrag!.noteId);
      if (note) lastLength = note.length;
      endDragCursor();
    }
    drawDrag = null;
  }

  // ── Moving the selection ──────────────────────────────────────────
  type MoveDrag = {
    pointerStepSnapped: number; pointerStepFree: number; pointerPitch: number;
    origins: Map<number, { start: number; pitch: number }>;
    primaryNoteId: number;   // the note actually clicked — drives preview retrigger + lastLength
    lastPreviewPitch: number;
    moved: boolean;
    collapseOnClick: boolean; // released without moving → select only primaryNoteId
  };
  // $state (not a plain let) so the highlightPitches derivation below can
  // react to the live drag.
  let moveDrag = $state<MoveDrag | null>(null);

  function clearSelection() {
    if (selectedNoteIds.size > 0) selectedNoteIds = new Set();
  }

  // Starts dragging every selected note together (or just `ids`, e.g. a
  // freshly placed note), `hit` being the one actually grabbed (drives
  // preview retrigger + lastLength).
  function startMoveDrag(hit: Note, x: number, step: number, pitch: number, collapseOnClick = false, ids?: number[]) {
    audioEngine.previewNote(channel, hit.pitch, hit);
    beginDragCursor('move');
    const moving = ids ? new Set(ids) : selectedNoteIds;
    const origins = new Map<number, { start: number; pitch: number }>();
    for (const n of channel.notes) if (moving.has(n.id)) origins.set(n.id, { start: n.start, pitch: n.pitch });
    moveDrag = {
      pointerStepSnapped: step, pointerStepFree: xToStepFree(x), pointerPitch: pitch,
      origins, primaryNoteId: hit.id, lastPreviewPitch: pitch, moved: false, collapseOnClick,
    };
  }

  function movePointermove(e: PointerEvent) {
    if (!moveDrag) return;
    if (e.buttons === 0) { commitMoveDrag(); return; }
    const { x, y } = localPos(e);
    const free = e.shiftKey;
    const current = free ? xToStepFree(x) : xToStep(x);
    const anchor = free ? moveDrag.pointerStepFree : moveDrag.pointerStepSnapped;
    const deltaStep = current - anchor;
    const pitch = yToPitch(y); // Y always snapped — Shift only frees the X axis
    const deltaPitch = pitch - moveDrag.pointerPitch;
    if (deltaStep !== 0 || deltaPitch !== 0) moveDrag.moved = true;
    for (const n of channel.notes) {
      const origin = moveDrag.origins.get(n.id);
      if (!origin) continue;
      n.start = Math.max(0, Math.min(patternLength - n.length, origin.start + deltaStep));
      n.pitch = clampPitch(origin.pitch + deltaPitch);
    }
    if (pitch !== moveDrag.lastPreviewPitch) {
      const primary = channel.notes.find(n => n.id === moveDrag!.primaryNoteId);
      if (primary) audioEngine.previewNote(channel, primary.pitch, primary);
      moveDrag.lastPreviewPitch = pitch;
    }
  }

  function commitMoveDrag() {
    if (moveDrag) {
      const primary = channel.notes.find(n => n.id === moveDrag!.primaryNoteId);
      if (primary) lastLength = primary.length;
      if (moveDrag.collapseOnClick && !moveDrag.moved) selectedNoteIds = new Set([moveDrag.primaryNoteId]);
      endDragCursor();
    }
    moveDrag = null;
  }

  // ── Marquee (drag-select) tool ────────────────────────────────────
  // Every drag draws a selection box, even one starting on a note (notes
  // are never moved from this tool). Plain replaces the selection, Shift
  // adds. Ctrl+drag runs this from every tool (see gridMousedown). A click without dragging selects the note under
  // the cursor (a zero-size box still overlaps it), or clears on empty space.
  let marquee = $state<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  let marqueeMode: 'replace' | 'add' = 'replace';

  function noteOverlapsRect(n: Note, minX: number, maxX: number, minY: number, maxY: number): boolean {
    const nx0 = stepToX(n.start), nx1 = stepToX(n.start + n.length);
    const ny0 = pitchToY(n.pitch), ny1 = ny0 + keyH();
    return nx0 <= maxX && nx1 > minX && ny0 <= maxY && ny1 > minY;
  }

  function startMarquee(e: MouseEvent, x: number, y: number) {
    marqueeMode = e.shiftKey ? 'add' : 'replace';
    marquee = { x0: x, y0: y, x1: x, y1: y };
    beginDragCursor('crosshair');
  }

  function marqueePointermove(e: PointerEvent) {
    if (!marquee) return;
    if (e.buttons === 0) { commitMarquee(); return; }
    const { x, y } = localPos(e);
    marquee = { ...marquee, x1: x, y1: y };
  }

  function commitMarquee() {
    if (!marquee) return;
    endDragCursor();
    const minX = Math.min(marquee.x0, marquee.x1), maxX = Math.max(marquee.x0, marquee.x1);
    const minY = Math.min(marquee.y0, marquee.y1), maxY = Math.max(marquee.y0, marquee.y1);
    const hits = channel.notes.filter(n => noteOverlapsRect(n, minX, maxX, minY, maxY));
    const hitIds = hits.map(n => n.id);
    // A plain click on a note is "clicking a note": it becomes the template too.
    if (maxX - minX < 3 && maxY - minY < 3 && hits.length === 1) adoptTemplate(hits[0]);
    if (marqueeMode === 'replace') {
      selectedNoteIds = new Set(hitIds);
    } else {
      selectedNoteIds = new Set([...selectedNoteIds, ...hitIds]);
    }
    marquee = null;
  }

  // ── Deleting: the Delete tool (left button) or right button anywhere ──
  // Pressing deletes the note under the cursor; keeping the button held
  // while dragging deletes every note the cursor passes over.
  let deleteButtons = 0; // e.buttons mask of the button driving the gesture; 0 = idle

  function deleteAt(x: number, y: number) {
    const hit = noteAt(xToStep(x), yToPitch(y));
    if (hit) deleteNote(hit);
  }

  function deleteDragStart(e: MouseEvent) {
    deleteButtons = e.button === 2 ? 2 : 1;
    beginDragCursor('var(--cursor-eraser)');
    const { x, y } = localPos(e);
    deleteAt(x, y);
  }

  // ── Piano-key highlight while a create/resize/move gesture is active ──
  // Delete-drag is intentionally excluded — only create/resize/
  // move gestures highlight a key.
  let activeGestureNoteIds = $derived.by(() => {
    if (drawDrag) return new Set<number>([drawDrag.noteId]);
    if (moveDrag) return new Set<number>(moveDrag.origins.keys());
    return new Set<number>();
  });

  let highlightPitches = $derived.by(() => {
    if (activeGestureNoteIds.size === 0) return new Set<number>();
    const s = new Set<number>();
    for (const n of channel.notes) if (activeGestureNoteIds.has(n.id)) s.add(n.pitch);
    return s;
  });

  function setsEqual(a: Set<number>, b: Set<number>): boolean {
    if (a.size !== b.size) return false;
    for (const v of a) if (!b.has(v)) return false;
    return true;
  }

  // $derived.by returns a new Set every recompute even when its contents are
  // unchanged (e.g. every pointermove tick during a pure length-resize,
  // where pitch never changes) — guard against forcing a needless re-render
  // of PianoKeys' 128 rows on every such tick.
  let lastSentHighlight = new Set<number>();
  $effect(() => {
    const next = highlightPitches;
    if (!setsEqual(next, lastSentHighlight)) {
      lastSentHighlight = next;
      onDragPitchesChange?.(next);
    }
  });

  // ── Dispatch by active tool ──────────────────────────────────────
  function gridMousedown(e: MouseEvent) {
    if (e.button === 2) { deleteDragStart(e); return; }
    if (e.button !== 0) return;
    // Ctrl+drag = the Marquee tool's drag, whatever tool is active and
    // whatever is under the cursor.
    if (tool === 'marquee' || e.ctrlKey || e.metaKey) { const { x, y } = localPos(e); startMarquee(e, x, y); return; }
    if (tool === 'draw') drawMousedown(e);
    else deleteDragStart(e);
  }
  function gridPointermove(e: PointerEvent) {
    if (deleteButtons) {
      if (!(e.buttons & deleteButtons)) { deleteButtons = 0; endDragCursor(); return; }
      const { x, y } = localPos(e);
      deleteAt(x, y);
      return;
    }
    if (drawDrag) drawPointermove(e);
    else if (moveDrag) movePointermove(e);
    else if (marquee) marqueePointermove(e);
  }
  function gridPointerup() {
    if (deleteButtons) { deleteButtons = 0; endDragCursor(); }
    commitDrawDrag();
    commitMoveDrag();
    commitMarquee();
  }
  // Hover cursor (no drag in progress): brush/eraser/crosshair per tool,
  // and in Draw, the resize arrows over a note's grabbable right edge.
  let hoverEdge = $state(false);

  function gridHover(e: PointerEvent) {
    if (tool !== 'draw' || e.buttons !== 0) { if (hoverEdge) hoverEdge = false; return; }
    const { x, y } = localPos(e);
    const hit = noteAt(xToStep(x), yToPitch(y));
    const onEdge = !!hit && x >= stepToX(hit.start + hit.length) - 6;
    if (onEdge !== hoverEdge) hoverEdge = onEdge;
  }

  function gridContextmenu(e: MouseEvent) {
    e.preventDefault();
  }

  // ── Zoom: ctrl/cmd+scroll and ctrl/cmd+=/- ────────────────────────
  function isTypingTarget(target: EventTarget | null): boolean {
    const t = target as HTMLElement | null;
    return t?.tagName === 'INPUT' || t?.tagName === 'TEXTAREA' || !!t?.isContentEditable;
  }

  async function reanchorZoom(anchorX: number, anchorY: number, zoomIn: boolean) {
    if (!viewportEl) return;
    const oldZoom = pianoRollZoom.zoom;
    const contentX = viewportEl.scrollLeft + anchorX;
    const contentY = viewportEl.scrollTop + anchorY;
    if (zoomIn) pianoRollZoom.zoomIn(); else pianoRollZoom.zoomOut();
    await tick(); // let grid-content's new width/height land in the DOM first
    const ratio = pianoRollZoom.zoom / oldZoom;
    viewportEl.scrollLeft = contentX * ratio - anchorX;
    viewportEl.scrollTop  = contentY * ratio - anchorY;
  }

  function zoomAt(clientX: number, clientY: number, zoomIn: boolean) {
    if (!viewportEl) return;
    const rect = viewportEl.getBoundingClientRect();
    reanchorZoom(clientX - rect.left, clientY - rect.top, zoomIn);
  }

  function zoomAtCenter(zoomIn: boolean) {
    if (!viewportEl) return;
    reanchorZoom(viewportEl.clientWidth / 2, viewportEl.clientHeight / 2, zoomIn);
  }

  function onGridWheel(e: WheelEvent) {
    if (!(e.ctrlKey || e.metaKey)) return; // let normal 2-axis scroll pass through untouched
    e.preventDefault();
    zoomAt(e.clientX, e.clientY, Math.sign(e.deltaY) < 0);
  }

  // ── Keyboard: delete / copy / paste / zoom ────────────────────────
  let clipboard: Note[] = [];

  function handleKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && (e.key === '=' || e.key === '+')) {
      if (isTypingTarget(e.target)) return;
      e.preventDefault();
      zoomAtCenter(true);
    } else if ((e.ctrlKey || e.metaKey) && e.key === '-') {
      if (isTypingTarget(e.target)) return;
      e.preventDefault();
      zoomAtCenter(false);
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
      if (isTypingTarget(e.target) || !windowFocus.isFrontmost('piano-roll')) return;
      selectedNoteIds = new Set(channel.notes.map(n => n.id));
      e.preventDefault();
    } else if (e.key === 'Escape') {
      if (isTypingTarget(e.target)) return;
      clearSelection();
    } else if (e.key === 'Delete' || e.key === 'Backspace') {
      // Only while the Piano Roll is frontmost — this is a window-level
      // listener, and a Delete aimed at another window (e.g. selected
      // patterns in the Playlist) must not touch the notes.
      if (isTypingTarget(e.target) || !windowFocus.isFrontmost('piano-roll')) return;
      if (selectedNoteIds.size === 0) {
        // Delete (not Backspace) with nothing selected clears every note in
        // this channel's pattern. Undoable like any edit (the history
        // watcher picks it up; Ctrl+Z flushes it first).
        if (e.key !== 'Delete' || channel.notes.length === 0) return;
        for (const n of channel.notes) markDeleted(delKey(n.id));
        channel.notes = [];
        e.preventDefault();
        return;
      }
      for (const id of selectedNoteIds) markDeleted(delKey(id));
      channel.notes = channel.notes.filter(n => !selectedNoteIds.has(n.id));
      selectedNoteIds = new Set();
      e.preventDefault();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c') {
      const sel = channel.notes.filter(n => selectedNoteIds.has(n.id));
      if (sel.length === 0) return;
      clipboard = sel.map(n => ({ ...n }));
      e.preventDefault();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'v') {
      if (clipboard.length === 0) return;
      const minStart = Math.min(...clipboard.map(n => n.start));
      const maxEnd   = Math.max(...clipboard.map(n => n.start + n.length));
      const offset   = maxEnd - minStart;
      const pasted: Note[] = clipboard.map(n => ({
        ...n,
        id: nextNoteId++,
        start: Math.max(0, Math.min(patternLength - n.length, n.start + offset)),
      }));
      channel.notes.push(...pasted);
      selectedNoteIds = new Set(pasted.map(n => n.id));
      e.preventDefault();
    }
  }

  // Mirrored for the playhead overlay (drawn outside the scroller).
  let scrollX = $state(0);

  function onGridScroll(e: Event) {
    const el = e.currentTarget as HTMLElement;
    scrollX = el.scrollLeft;
    onScroll(el.scrollLeft, el.scrollTop);
  }

  // Lets a parent (e.g. the minimap/overview panel) teleport this viewport's
  // scroll position from outside — setting scrollLeft/scrollTop fires the
  // native onscroll handler above, so callers get their own scroll state
  // (and any overlay derived from it) corrected to the real post-clamp value
  // for free, with no extra plumbing.
  export function scrollTo(x: number, y: number) {
    if (!viewportEl) return;
    viewportEl.scrollLeft = x;
    viewportEl.scrollTop = y;
  }

  onMount(() => {
    if (!viewportEl) return;
    const target = Math.max(0, pitchToY(DEFAULT_CENTER_PITCH) - viewportEl.clientHeight / 2);
    viewportEl.scrollTop = target;
    onScroll(viewportEl.scrollLeft, viewportEl.scrollTop);
    onViewportResize?.(viewportEl.clientWidth);
  });

  $effect(() => {
    if (!viewportEl) return;
    const el = viewportEl;
    const ro = new ResizeObserver(() => onViewportResize?.(el.clientWidth));
    ro.observe(el);
    return () => ro.disconnect();
  });
</script>

<svelte:window onpointermove={gridPointermove} onpointerup={gridPointerup} onkeydown={handleKeydown} />

<div class="grid-wrap">
<div class="grid-viewport" bind:this={viewportEl} onscroll={onGridScroll} onwheel={onGridWheel} role="grid" aria-label="Note grid" tabindex="-1">
  <div
    class="grid-content"
    class:grid-content--draw={tool === 'draw'}
    class:grid-content--delete={tool === 'delete'}
    class:grid-content--edge={tool === 'draw' && hoverEdge}
    onpointermove={gridHover}
    onpointerleave={() => (hoverEdge = false)}
    bind:this={contentEl}
    style="width:{patternWidth(patternLength)}px; height:{gridTotalH()}px;"
    onmousedown={gridMousedown}
    oncontextmenu={gridContextmenu}
    role="presentation"
  >
    {#each pitches as p (p)}
      <div
        class="grid-row"
        class:grid-row--black={isBlackKey(p)}
        class:grid-row--c={isCNote(p)}
        style="top:{pitchToY(p)}px; height:{keyH()}px;"
      ></div>
    {/each}

    {#each Array(patternLength) as _, i}
      <div
        class="grid-col"
        class:grid-col--orange={Math.floor(i / 4) % 2 === 1}
        style="left:{stepToX(i)}px; width:{stepW()}px;"
      ></div>
    {/each}

    {#if midiPitches}
      {#each [...midiPitches] as [p, v] (p)}
        <div class="midi-row-glow" style="top:{pitchToY(p)}px; height:{keyH()}px; --glow:{v};"></div>
      {/each}
    {/if}

    {#key channel.id}
      {#each channel.notes as note (note.id)}
        {@const color = noteColorStore.colorOf(note.color)}
        <div
          class="note"
          class:note--selected={selectedNoteIds.has(note.id)}
          class:note--dimmed={selectedNoteIds.size > 0 && !selectedNoteIds.has(note.id)}
          class:note--slide={note.slide}
          style="left:{stepToX(note.start)}px; top:{pitchToY(note.pitch)}px; width:{note.length * stepW() - 1}px; height:{keyH() - 1}px; --note-color:{color}; --note-text:{note.slide ? 'rgba(255,255,255,0.85)' : readableTextOn(color)};"
          data-del-key={delKey(note.id)}
          out:deleteOut
        >
          {#if note.slide}
            <!-- Slide (pitch-bend) note marker: a rising ramp -->
            <svg class="slide-icon" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 9L9 1V9z" /></svg>
          {/if}
          <span class="note-label">{pitchName(note.pitch)}</span>
          <span class="note-velocity" style="width:{Math.max(0, Math.min(1, note.velocity)) * 100}%"></span>
        </div>
      {/each}
    {/key}

    {#if marquee}
      {@const mx = Math.min(marquee.x0, marquee.x1)}
      {@const my = Math.min(marquee.y0, marquee.y1)}
      {@const mw = Math.abs(marquee.x1 - marquee.x0)}
      {@const mh = Math.abs(marquee.y1 - marquee.y0)}
      <div class="marquee" style="left:{mx}px; top:{my}px; width:{mw}px; height:{mh}px;"></div>
    {/if}
  </div>
</div>
<!-- Playhead step band + smooth line, in an overlay *outside* the scroller
     (offset by scrollX) and moved with `transform`. Anything that changes
     every frame inside a scroller makes WebKitGTK re-render it together
     with every floating window stacked over it — see the Playlist's
     playhead overlay and CLAUDE.md's "Per-frame updates" note. -->
<div class="playhead-overlay">
  {#if activeStep >= 0}
    <div class="playhead" style="width:{stepW()}px; transform: translateX({stepToX(activeStep) - scrollX}px);"></div>
  {/if}
  {#if currentStepFraction >= 0}
    <div class="playhead-line" style="transform: translateX({currentStepFraction * stepW() - scrollX}px);"></div>
  {/if}
</div>
</div>

<style>
  .grid-wrap {
    position: relative;
    height: 100%;
  }

  .playhead-overlay {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    z-index: 4;
  }

  .grid-viewport {
    height: 100%;
    overflow: auto;
    overflow-anchor: none;
    position: relative;
    background: #191c25;
  }

  /* Marquee tool keeps the crosshair; drag cursors come from dragCursor.ts. */
  .grid-content {
    position: relative;
    cursor: crosshair;
  }

  .grid-content--draw { cursor: var(--cursor-brush); }
  .grid-content--delete { cursor: var(--cursor-eraser); }
  .grid-content--edge { cursor: col-resize; }

  .grid-row {
    position: absolute;
    left: 0;
    right: 0;
    box-sizing: border-box;
    border-bottom: 1px solid rgba(255,255,255,0.03);
  }

  .grid-row--black { background: rgba(0,0,0,0.22); }
  .grid-row--c { border-bottom: 1px solid rgba(255,255,255,0.1); }

  .grid-col {
    position: absolute;
    top: 0;
    bottom: 0;
    box-sizing: border-box;
    border-right: 1px solid rgba(255,255,255,0.04);
  }

  .grid-col--orange { background: rgba(230,176,127,0.03); }

  .playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    background: rgba(255,255,255,0.08);
    outline: 1px solid rgba(255,255,255,0.3);
    will-change: transform;
  }

  .note {
    position: absolute;
    box-sizing: border-box;
    background: var(--note-color, var(--accent));
    border: 1px solid rgba(0,0,0,0.4);
    border-radius: 8px;
    z-index: 3;
    display: flex;
    align-items: center;
    overflow: hidden;
  }

  .note-label {
    padding-left: 3px;
    font-size: 9px;
    font-family: 'DM Mono', monospace;
    color: var(--note-text, rgba(0,0,0,0.55));
    line-height: 1;
    white-space: nowrap;
    pointer-events: none;
    user-select: none;
  }

  /* Velocity indicator: a darker bar (accent mixed with black) along the note's bottom edge,
     as wide as the note is long at velocity 1 (same 0–1 scale as the
     velocity lane). */
  .note-velocity {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 3px;
    background: color-mix(in srgb, var(--note-color, var(--accent)) 45%, #080b15);
    pointer-events: none;
  }

  /* Slide notes: hollow-ish (translucent fill, solid colored border) with a
     ramp glyph, so they read as "bend instructions" rather than sounding
     notes. */
  .note--slide {
    background: color-mix(in srgb, var(--note-color) 28%, #191c25);
    border: 1px solid var(--note-color);
  }

  .slide-icon {
    flex-shrink: 0;
    width: 9px;
    height: 9px;
    margin-left: 3px;
    fill: var(--note-color);
    pointer-events: none;
  }

  .note--slide .note-label { padding-left: 2px; }

  .midi-row-glow {
    position: absolute;
    left: 0;
    right: 0;
    background: var(--accent);
    opacity: calc(0.08 + 0.14 * var(--glow));
    box-shadow: 0 0 8px var(--accent);
    pointer-events: none;
  }

  .playhead-line {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 1px;
    background: #e4e5e6;
    box-shadow: 0 0 4px rgba(255,255,255,0.6);
    will-change: transform;
  }

  /* Selection has to read clearly on any of the 15 palette colors, so it
     isn't a color swap: selected notes get a bright white rim + glow and a
     diagonal stripe overlay, and while anything is selected every other
     note fades back. */
  .note--selected {
    border: 2px solid #ffffff;
    box-shadow: 0 0 0 1px rgba(0,0,0,0.6), 0 0 8px rgba(255,255,255,0.55);
    background-image: repeating-linear-gradient(
      -45deg,
      rgba(255,255,255,0.32) 0 3px,
      transparent 3px 7px
    );
    z-index: 5;
  }

  .note--dimmed { opacity: 0.4; }

  .marquee {
    position: absolute;
    background: rgba(230,176,127,0.15);
    border: 1px solid var(--accent, #e6b07f);
    z-index: 4;
    pointer-events: none;
  }
</style>
