// Playlist block geometry + the "no overlapping blocks on a track" rule,
// shared by drawing (placing a pattern), live-moving a block and dropping
// a sample clip (Playlist.svelte).
import type { Placement } from './types';

// A block's length in beats: a pattern's effective (content-floored)
// length — the same patternStore.effectiveLengths value PlaylistTrackRow
// draws and Song mode schedules — or a clip's own length.
export function placementLength(p: Placement, effectiveLengths: ReadonlyMap<number, number>): number {
  return p.kind === 'pattern' ? effectiveLengths.get(p.patternId) ?? 0 : p.lengthBeats;
}

const EPS = 1e-6;

// The start beat closest to `desired` at which a block of `length` fits on
// `trackId` without overlapping any other block there (`ignoreId` = the
// block being moved), or null if nothing fits within `maxDistance` beats.
// Candidates are the desired spot itself and the slots flush against each
// neighbour's edges, so a block dragged into another one sticks to its
// side instead of refusing to move.
export function nearestFreeStart(
  placements: readonly Placement[],
  effectiveLengths: ReadonlyMap<number, number>,
  trackId: number,
  desired: number,
  length: number,
  ignoreId: number | null = null,
  maxDistance = Infinity,
): number | null {
  const others = placements
    .filter(p => p.trackId === trackId && p.id !== ignoreId)
    .map(p => ({ start: p.startBeat, end: p.startBeat + placementLength(p, effectiveLengths) }));
  const fits = (start: number) =>
    start >= -EPS && others.every(o => start + length <= o.start + EPS || start >= o.end - EPS);

  const candidates = [Math.max(0, desired)];
  for (const o of others) candidates.push(o.start - length, o.end);
  let best: number | null = null;
  for (const c of candidates) {
    const start = Math.max(0, c);
    if (!fits(start)) continue;
    const d = Math.abs(start - desired);
    if (d <= maxDistance && (best === null || d < Math.abs(best - desired))) best = start;
  }
  return best;
}

// Whether a rigid group move fits: every moved block (`targets`, new track +
// start) stays at beat ≥ 0 and overlaps no block outside the group. The
// group can't overlap itself — it moves as one, by the same offsets.
export function groupFits(
  placements: readonly Placement[],
  effectiveLengths: ReadonlyMap<number, number>,
  targets: readonly { id: number; trackId: number; start: number }[],
): boolean {
  const moving = new Set(targets.map(t => t.id));
  const length = new Map(placements.map(p => [p.id, placementLength(p, effectiveLengths)]));
  const others = placements.filter(p => !moving.has(p.id));
  return targets.every(t => {
    const len = length.get(t.id) ?? 0;
    return t.start >= -EPS && others.every(o =>
      o.trackId !== t.trackId
      || t.start + len <= o.startBeat + EPS
      || t.start >= o.startBeat + (length.get(o.id) ?? 0) - EPS);
  });
}
