import type { PatternData } from './types';
import { channelStore } from './channelStore.svelte';
import { playback } from './playbackStore.svelte';
import { patternContentLength } from './channelLoop';
import { profiler } from './debug/profiler';

const PATTERN_COLORS = [
  '#e6b07f', '#d3a76f', '#6c91b1', '#c17f76',
  '#a590d6', '#60a9a1', '#c9bb62', '#b96e8d',
];

let nextId = 2;

export interface PatternStoreExport {
  patterns: PatternData[];
  selectedPatternId: number;
}

function makePattern(id: number): PatternData {
  return {
    id,
    name: `Pattern ${id}`,
    color: PATTERN_COLORS[(id - 1) % PATTERN_COLORS.length],
    lengthBeats: 4,
  };
}

class PatternStore {
  patterns = $state<PatternData[]>([makePattern(1)]);
  #selectedPatternId = $state(1);

  get selectedPatternId(): number {
    return this.#selectedPatternId;
  }

  // Switching the selected pattern hands off to channelStore so channels
  // (and their sample/mute/pan/volume settings) stay put while their
  // step/note content swaps to whatever was last drawn in that pattern.
  set selectedPatternId(id: number) {
    if (id === this.#selectedPatternId) return;
    channelStore.switchPattern(this.#selectedPatternId, id);
    this.#selectedPatternId = id;
    playback.restartFromZero();
  }

  get selectedPattern(): PatternData | null {
    return this.patterns.find(p => p.id === this.#selectedPatternId) ?? null;
  }

  // Every pattern's actual Song-mode length in beats, keyed by pattern id:
  // its own manually-set lengthBeats, floored up to "as long as its longest
  // channel's own content" (see channelLoop.ts's patternContentLength) so
  // nothing drawn in the Channel Rack/Piano Roll is ever silently truncated.
  // A single `$derived.by` (not a plain getter) so this only re-scans when
  // patterns/channel content actually change — both `songTimeline.svelte.ts`
  // (audio scheduling) and `PlaylistTrackRow.svelte` (the block's on-screen
  // width) read this same memoized map instead of each independently
  // re-running the scan, which is what made hitting Play in Song mode "lag
  // like crazy": the same per-channel bar-scan was being redone from
  // scratch dozens of times a second.
  effectiveLengths = $derived.by((): Map<number, number> => {
    profiler.count('derived.effectiveLengths');
    const barLength = channelStore.barLength;
    const map = new Map<number, number>();
    for (const p of this.patterns) {
      const channels = channelStore.getPatternChannels(p.id, this.#selectedPatternId);
      const contentSteps = patternContentLength(channels, barLength);
      map.set(p.id, Math.max(p.lengthBeats, contentSteps / 4));
    }
    return map;
  });

  // The name addPattern() would auto-assign — the default in the
  // new-pattern name prompt (NewPatternModal.svelte).
  get nextPatternName(): string {
    return makePattern(nextId).name;
  }

  addPattern(name?: string) {
    const pattern = makePattern(nextId++);
    if (name?.trim()) pattern.name = name.trim();
    this.patterns.push(pattern);
    this.selectedPatternId = pattern.id;
    return pattern;
  }

  // Deletes patterns and their per-pattern channel content. If the active
  // pattern goes, the nearest surviving one (next in the list, else the
  // previous) becomes active first; deleting every pattern leaves one fresh
  // blank pattern, since the rest of the app always needs an active one.
  // Placements of the deleted patterns are the caller's job
  // (placementStore doesn't belong to this store).
  removePatterns(ids: Iterable<number>) {
    const remove = new Set(ids);
    if (remove.size === 0) return;
    if (this.patterns.every(p => remove.has(p.id))) this.patterns.push(makePattern(nextId++));
    if (remove.has(this.#selectedPatternId)) {
      const idx = this.patterns.findIndex(p => p.id === this.#selectedPatternId);
      const next = this.patterns.slice(idx + 1).find(p => !remove.has(p.id))
        ?? [...this.patterns.slice(0, idx)].reverse().find(p => !remove.has(p.id))!;
      this.selectedPatternId = next.id;
    }
    this.patterns = this.patterns.filter(p => !remove.has(p.id));
    for (const id of remove) channelStore.removePatternContent(id);
  }

  exportState(): PatternStoreExport {
    return {
      patterns: this.patterns.map(p => ({ ...p })),
      selectedPatternId: this.#selectedPatternId,
    };
  }

  // Bypasses the public selectedPatternId setter — it would call
  // channelStore.switchPattern, which must not run during a full-state
  // import since channelStore.importState already restores every pattern's
  // content wholesale.
  importState(data: PatternStoreExport) {
    this.patterns = data.patterns.map(p => ({ ...p }));
    this.#selectedPatternId = data.selectedPatternId;
    nextId = this.patterns.reduce((m, p) => Math.max(m, p.id), 0) + 1;
  }

  resetToDefault() {
    this.patterns = [makePattern(1)];
    this.#selectedPatternId = 1;
    nextId = 2;
  }
}

export const patternStore = new PatternStore();
