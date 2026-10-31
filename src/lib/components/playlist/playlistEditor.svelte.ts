// Pointer/keyboard editing of placed blocks on the Playlist timeline —
// the Piano Roll's three tools (editTools.ts) applied to placements:
//   draw    — click empty lane space places the active pattern (and keeps
//             moving it while held); click a block selects it (Shift
//             adds) and drags the selection; Shift+drag on empty space
//             marquee-selects.
//   delete  — click/drag erases blocks.
//   marquee — every drag is a selection box.
// Ctrl+drag is the marquee tool's drag in every tool, on blocks too.
// The right button deletes / drag-deletes in every tool. Blocks never
// overlap: a single block uses nearestFreeStart (sticks flush to a
// neighbour), a multi-block selection moves rigidly and simply stops at
// the last position that fits (groupFits).
import { SvelteSet } from 'svelte/reactivity';
import { placementStore } from '$lib/placementStore.svelte';
import { patternStore } from '$lib/patternStore.svelte';
import { markDeleted } from '$lib/deleteFx';
import { beginDragCursor, endDragCursor } from '$lib/dragCursor';
import { groupFits, nearestFreeStart, placementLength } from '$lib/placementLayout';
import type { EditTool } from '$lib/editTools';
import type { Placement } from '$lib/types';

interface Deps {
  grid: () => HTMLDivElement | undefined;
  pixelsPerBeat: () => number;
  trackIds: () => number[];
  trackPanelW: number;
}

interface MoveDrag {
  anchorBeat: number;
  anchorTrackIdx: number;
  trackDelta: number;
  orig: { id: number; start: number; trackIdx: number }[];
  moved: boolean;
  collapseTo: number | null; // plain click inside a multi-selection: narrow to it if nothing moved
}

type MarqueeMode = 'replace' | 'add';

// Grid-local px (the box scrolls with the content).
export interface Marquee { x0: number; y0: number; x1: number; y1: number }

// The start beat closest to `desired` where a block of `length` fits on a
// track (see placementLayout.ts).
export function freeStart(trackId: number, desired: number, length: number, ignoreId: number | null, maxDistance = Infinity) {
  return nearestFreeStart(placementStore.placements, patternStore.effectiveLengths, trackId, desired, length, ignoreId, maxDistance);
}

function findTrackIdAt(x: number, y: number): number | null {
  const el = document.elementsFromPoint(x, y).find(el => el.hasAttribute('data-track-id'));
  return el ? Number(el.getAttribute('data-track-id')) : null;
}

function placementIdAt(x: number, y: number): number | null {
  const el = document.elementsFromPoint(x, y).find(el => el.hasAttribute('data-placement-id'));
  return el ? Number(el.getAttribute('data-placement-id')) : null;
}

const findPlacement = (id: number) => placementStore.placements.find(p => p.id === id);

export class PlaylistEditor {
  tool = $state<EditTool>('draw');
  readonly selected = new SvelteSet<number>();
  moveDrag = $state<MoveDrag | null>(null);
  marquee = $state<Marquee | null>(null);
  movingIds = $derived(new Set(this.moveDrag?.orig.map(o => o.id) ?? []));

  #deps: Deps;
  #marqueeMode: MarqueeMode = 'replace';
  #marqueeBase: number[] = [];
  #deleteBit = 0; // the e.buttons bit driving a delete drag (1 left, 2 right), 0 = none

  constructor(deps: Deps) {
    this.#deps = deps;
  }

  #beatAt(clientX: number): number {
    const rect = this.#deps.grid()!.getBoundingClientRect();
    return (clientX - rect.left - this.#deps.trackPanelW) / this.#deps.pixelsPerBeat();
  }

  #select(ids: Iterable<number>) {
    this.selected.clear();
    for (const id of ids) this.selected.add(id);
  }

  // Clicking a placed pattern makes it the one you draw next (like the
  // Piano Roll adopting a clicked note's color/length).
  #adopt(placement: Placement) {
    if (placement.kind === 'pattern' && placement.patternId !== patternStore.selectedPatternId) {
      patternStore.selectedPatternId = placement.patternId;
    }
  }

  // Drops selected ids whose placement is gone (deleted, undone, loaded).
  prune() {
    const live = new Set(placementStore.placements.map(p => p.id));
    for (const id of this.selected) if (!live.has(id)) this.selected.delete(id);
  }

  selectAll() {
    this.#select(placementStore.placements.map(p => p.id));
  }

  deleteSelected() {
    if (this.selected.size === 0) return;
    const ids = new Set(this.selected);
    for (const id of ids) markDeleted(`placement:${id}`); // fade + red ring, see deleteFx.ts
    placementStore.placements = placementStore.placements.filter(p => !ids.has(p.id));
    this.selected.clear();
  }

  // ── Pointer entry points (from PlaylistTrackRow) ────────────────────

  // Left button on empty lane space.
  onLanePointerDown(e: PointerEvent, trackId: number) {
    if (e.button !== 0 || !this.#deps.grid()) return;
    if (this.tool === 'marquee' || e.ctrlKey || e.metaKey) { this.#startMarquee(e); return; }
    if (this.tool === 'delete') { this.#startDelete(e, 1); return; }
    if (e.shiftKey) { this.#startMarquee(e); return; }
    // Draw: place the active pattern in the clicked beat cell — or flush
    // against a neighbour if it wouldn't fit there, but never more than
    // its own length away — and keep moving it while the button is held.
    this.selected.clear();
    const pattern = patternStore.selectedPattern;
    if (!pattern) return;
    const rawBeat = this.#beatAt(e.clientX);
    if (rawBeat < 0) return;
    const length = patternStore.effectiveLengths.get(pattern.id) ?? pattern.lengthBeats;
    const start = freeStart(trackId, Math.floor(rawBeat), length, null, length);
    if (start === null) return;
    const placement = placementStore.addPattern(pattern.id, trackId, start);
    this.#startMove(e, [placement.id], null);
  }

  // Left button on a placed block.
  onPlacementPointerDown(e: PointerEvent, placement: Placement) {
    if (e.button !== 0) return;
    e.stopPropagation(); // not a lane click
    if (!this.#deps.grid()) return;
    if (this.tool === 'marquee' || e.ctrlKey || e.metaKey) { this.#startMarquee(e); return; }
    if (this.tool === 'delete') { this.#startDelete(e, 1); return; }
    this.#adopt(placement);
    const id = placement.id;
    let collapseTo: number | null = null;
    if (e.shiftKey) this.selected.add(id);
    else if (!this.selected.has(id)) this.#select([id]);
    else if (this.selected.size > 1) collapseTo = id;
    this.#startMove(e, [...this.selected], collapseTo);
  }

  // Right button anywhere in a track row, in every tool.
  onDeletePointerDown(e: PointerEvent) {
    this.#startDelete(e, 2);
  }

  onWindowPointerMove(e: PointerEvent) {
    if (this.#deleteBit) {
      if (!(e.buttons & this.#deleteBit)) { this.#endDelete(); return; }
      this.#deleteAt(e.clientX, e.clientY);
    } else if (this.marquee) {
      if (e.buttons === 0) { this.#endMarquee(); return; }
      this.#updateMarquee(e);
    } else if (this.moveDrag) {
      if (e.buttons === 0) { this.#endMove(); return; }
      this.#updateMove(e);
    }
  }

  onWindowPointerUp() {
    this.#endDelete();
    this.#endMarquee();
    this.#endMove();
  }

  // ── Moving ──────────────────────────────────────────────────────────
  // Live (blocks follow the pointer), snapped by the pointer's whole-beat
  // delta from where it grabbed (blocks keep their offset; Shift during
  // the drag = unsnapped); vertical movement shifts tracks.

  #startMove(e: PointerEvent, ids: number[], collapseTo: number | null) {
    const trackIds = this.#deps.trackIds();
    const orig = ids.flatMap(id => {
      const p = findPlacement(id);
      return p ? [{ id, start: p.startBeat, trackIdx: trackIds.indexOf(p.trackId) }] : [];
    });
    if (orig.length === 0) return;
    const anchorTrack = findTrackIdAt(e.clientX, e.clientY);
    this.moveDrag = {
      anchorBeat: this.#beatAt(e.clientX),
      anchorTrackIdx: anchorTrack === null ? orig[0].trackIdx : trackIds.indexOf(anchorTrack),
      trackDelta: 0,
      orig,
      moved: false,
      collapseTo,
    };
    beginDragCursor('move');
  }

  #updateMove(e: PointerEvent) {
    const d = this.moveDrag!;
    const trackIds = this.#deps.trackIds();
    const now = this.#beatAt(e.clientX);
    let delta = e.shiftKey ? now - d.anchorBeat : Math.floor(now) - Math.floor(d.anchorBeat);
    const hover = findTrackIdAt(e.clientX, e.clientY);
    if (hover !== null) d.trackDelta = trackIds.indexOf(hover) - d.anchorTrackIdx;
    // Keep the whole group on the timeline.
    const minIdx = Math.min(...d.orig.map(o => o.trackIdx));
    const maxIdx = Math.max(...d.orig.map(o => o.trackIdx));
    const trackDelta = Math.max(-minIdx, Math.min(trackIds.length - 1 - maxIdx, d.trackDelta));
    delta = Math.max(delta, -Math.min(...d.orig.map(o => o.start)));

    if (d.orig.length === 1) {
      const o = d.orig[0];
      const placement = findPlacement(o.id);
      if (!placement) { this.#endMove(); return; }
      const trackId = trackIds[o.trackIdx + trackDelta];
      const length = placementLength(placement, patternStore.effectiveLengths);
      const start = freeStart(trackId, o.start + delta, length, o.id);
      if (start === null || (start === placement.startBeat && trackId === placement.trackId)) return;
      placementStore.update(o.id, { trackId, startBeat: start });
      d.moved = true;
      return;
    }

    const targets = d.orig.map(o => ({ id: o.id, trackId: trackIds[o.trackIdx + trackDelta], start: o.start + delta }));
    if (!groupFits(placementStore.placements, patternStore.effectiveLengths, targets)) return;
    const byId = new Map(targets.map(t => [t.id, t]));
    let changed = false;
    placementStore.placements = placementStore.placements.map(p => {
      const t = byId.get(p.id);
      if (!t || (t.start === p.startBeat && t.trackId === p.trackId)) return p;
      changed = true;
      return { ...p, trackId: t.trackId, startBeat: t.start };
    });
    if (changed) d.moved = true;
  }

  #endMove() {
    const d = this.moveDrag;
    if (!d) return;
    endDragCursor();
    if (!d.moved && d.collapseTo !== null) this.#select([d.collapseTo]);
    this.moveDrag = null;
  }

  // ── Marquee ─────────────────────────────────────────────────────────
  // Plain = replace, Shift = add (same as the Piano Roll).
  // A click without dragging selects (and adopts) the block under the
  // pointer, or clears on empty space.

  #local(e: PointerEvent) {
    const rect = this.#deps.grid()!.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  }

  #startMarquee(e: PointerEvent) {
    const { x, y } = this.#local(e);
    this.#marqueeMode = e.shiftKey ? 'add' : 'replace';
    this.#marqueeBase = this.#marqueeMode === 'replace' ? [] : [...this.selected];
    this.marquee = { x0: x, y0: y, x1: x, y1: y };
    this.#applyMarquee();
  }

  #updateMarquee(e: PointerEvent) {
    const { x, y } = this.#local(e);
    this.marquee = { ...this.marquee!, x1: x, y1: y };
    this.#applyMarquee();
  }

  // Hit-tests the rendered blocks (a zero-size box still hits the block
  // it's on, so a plain click selects it).
  #applyMarquee() {
    const m = this.marquee!;
    const grid = this.#deps.grid()!;
    const g = grid.getBoundingClientRect();
    const left = g.left + Math.min(m.x0, m.x1), right = g.left + Math.max(m.x0, m.x1);
    const top = g.top + Math.min(m.y0, m.y1), bottom = g.top + Math.max(m.y0, m.y1);
    const hits: number[] = [];
    for (const el of grid.querySelectorAll<HTMLElement>('[data-placement-id]')) {
      const id = Number(el.dataset.placementId);
      if (!findPlacement(id)) continue; // still fading out
      const r = el.getBoundingClientRect();
      if (r.left <= right && r.right >= left && r.top <= bottom && r.bottom >= top) hits.push(id);
    }
    this.#select(new Set([...this.#marqueeBase, ...hits]));
  }

  #endMarquee() {
    const m = this.marquee;
    if (!m) return;
    this.marquee = null;
    const isClick = Math.abs(m.x1 - m.x0) < 3 && Math.abs(m.y1 - m.y0) < 3;
    if (!isClick) return;
    const g = this.#deps.grid()!.getBoundingClientRect();
    const id = placementIdAt(g.left + m.x0, g.top + m.y0);
    const placement = id === null ? undefined : findPlacement(id);
    if (placement) this.#adopt(placement);
  }

  // ── Deleting ────────────────────────────────────────────────────────
  // Press deletes the block under the pointer; while the button stays held
  // every block passed over goes too. Hit-tested via data-placement-id (a
  // block fading out has pointer-events: none, so it can't be hit twice).

  #startDelete(e: PointerEvent, bit: number) {
    this.#deleteBit = bit;
    beginDragCursor('not-allowed');
    this.#deleteAt(e.clientX, e.clientY);
  }

  #deleteAt(x: number, y: number) {
    const id = placementIdAt(x, y);
    if (id === null || !findPlacement(id)) return;
    markDeleted(`placement:${id}`);
    placementStore.remove(id);
    this.selected.delete(id);
  }

  #endDelete() {
    if (this.#deleteBit) endDragCursor();
    this.#deleteBit = 0;
  }
}
