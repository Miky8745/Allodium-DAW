// Pure helpers translating placementStore's beat-based Playlist placements
// into the step-based (16th-note) units audioEngine's scheduler already
// works in everywhere else — the fixed 4-steps-per-beat convention used
// throughout this codebase. Read by audioEngine's song-mode ticking (via
// playbackStore) and nothing else.
//
// `.svelte.ts` (not plain `.ts`) specifically so `placements`/`lengthSteps`
// below can be `$derived` — audioEngine's scheduler reads these via plain
// closures many times a second (tick() runs every 25ms, and used to call
// this module's functions fresh every time), and each read used to redo a
// full per-placement content scan (patternContentLength, which itself
// bar-scans every channel's notes/steps — see channelLoop.ts). That's what
// made hitting Play in Song mode "lag like crazy": real work was repeated
// 40+ times a second even though the underlying content hadn't changed.
// $derived memoizes on Svelte's own signal graph — it only re-runs the
// computation when one of the $state values it reads (placementStore.
// placements, patternStore.patterns, or any channel's steps/notes) actually
// changes, so a steady-state read during playback is just a cached-value
// return, no re-scan.
import { placementStore } from './placementStore.svelte';
import { patternStore } from './patternStore.svelte';
import { channelStore } from './channelStore.svelte';
import { profiler } from './debug/profiler';

export interface SongPlacement {
  id: number;
  kind: 'pattern' | 'clip';
  trackId: number;
  startStep: number;
  lengthSteps: number;
  patternId?: number;   // kind === 'pattern'
  samplePath?: string;  // kind === 'clip'
}

class SongTimeline {
  placements = $derived.by((): SongPlacement[] => {
    profiler.count('derived.songPlacements');
    const result: SongPlacement[] = [];
    for (const p of placementStore.placements) {
      const startStep = p.startBeat * 4;
      if (p.kind === 'pattern') {
        const pattern = patternStore.patterns.find(pt => pt.id === p.patternId);
        if (!pattern) continue; // dangling reference — mirrors Playlist.svelte's own {#if pattern} guard
        // patternStore.effectiveLengths is itself a memoized $derived (see
        // its own doc comment) covering both the content-driven floor and
        // pattern.lengthBeats — reused here rather than re-scanning, so the
        // same content scan isn't duplicated between audio scheduling and
        // the Playlist's own block-width rendering.
        const lengthBeats = patternStore.effectiveLengths.get(p.patternId!) ?? pattern.lengthBeats;
        const lengthSteps = lengthBeats * 4;
        result.push({ id: p.id, kind: 'pattern', trackId: p.trackId, startStep, lengthSteps, patternId: p.patternId });
      } else {
        result.push({ id: p.id, kind: 'clip', trackId: p.trackId, startStep, lengthSteps: p.lengthBeats * 4, samplePath: p.samplePath });
      }
    }
    return result;
  });

  // Total song length in steps — the point song-mode playback stops at (see
  // audioEngine.ts's tick(), stop-at-end). Rounded up to a whole bar so the
  // arrangement never cuts off mid-bar; floored at one bar for an empty
  // arrangement so the loop bound is never zero/negative.
  lengthSteps = $derived.by((): number => {
    const barLength = channelStore.barLength;
    let maxEnd = 0;
    for (const sp of this.placements) maxEnd = Math.max(maxEnd, sp.startStep + sp.lengthSteps);
    if (maxEnd === 0) return barLength;
    return Math.ceil(maxEnd / barLength) * barLength;
  });
}

export const songTimeline = new SongTimeline();

// Function-style wrappers so audioEngine/playbackStore keep passing plain
// closures (`() => getSongPlacements()`) — unchanged call sites, just now
// backed by a memoized reactive value instead of a from-scratch recompute.
export function getSongPlacements(): SongPlacement[] {
  return songTimeline.placements;
}

export function getSongLengthSteps(): number {
  return songTimeline.lengthSteps;
}
