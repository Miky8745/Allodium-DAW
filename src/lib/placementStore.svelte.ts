import type { Placement } from './types';

let nextId = 1;

class PlacementStore {
  placements = $state<Placement[]>([]);

  addPattern(patternId: number, trackId: number, startBeat: number): Placement {
    const placement: Placement = { id: nextId++, kind: 'pattern', patternId, trackId, startBeat, samplePath: '', lengthBeats: 0 };
    this.placements.push(placement);
    return placement;
  }

  addClip(samplePath: string, trackId: number, startBeat: number, lengthBeats: number): Placement {
    const placement: Placement = { id: nextId++, kind: 'clip', patternId: 0, trackId, startBeat, samplePath, lengthBeats };
    this.placements.push(placement);
    return placement;
  }

  // lengthBeats is only ever patched here for a clip placement, once its
  // sample's real decoded duration is known (see Playlist.svelte's
  // onClipDrop — the block appears immediately at a fallback width, then
  // this corrects it).
  update(id: number, changes: Partial<Pick<Placement, 'trackId' | 'startBeat' | 'lengthBeats'>>) {
    this.placements = this.placements.map(p => (p.id === id ? { ...p, ...changes } : p));
  }

  remove(id: number) {
    this.placements = this.placements.filter(p => p.id !== id);
  }

  exportState(): Placement[] {
    return this.placements.map(p => ({ ...p }));
  }

  importState(data: Placement[]) {
    this.placements = data.map(p => ({ ...p }));
    nextId = this.placements.reduce((m, p) => Math.max(m, p.id), 0) + 1;
  }

  resetToDefault() {
    this.placements = [];
    nextId = 1;
  }
}

export const placementStore = new PlacementStore();
