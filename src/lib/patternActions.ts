import { patternStore } from './patternStore.svelte';
import { placementStore } from './placementStore.svelte';
import { markDeleted } from './deleteFx';

// Deletes patterns everywhere: their Playlist placements and their cards
// (both with the deletion effect), then the patterns themselves plus their
// channel content (patternStore.removePatterns). One undo step, like any
// other edit.
export function deletePatterns(ids: Iterable<number>) {
  const remove = new Set(ids);
  if (remove.size === 0) return;
  for (const p of placementStore.placements) {
    if (p.kind === 'pattern' && remove.has(p.patternId)) markDeleted(`placement:${p.id}`);
  }
  placementStore.placements = placementStore.placements.filter(p => !(p.kind === 'pattern' && remove.has(p.patternId)));
  for (const id of remove) markDeleted(`pattern:${id}`);
  patternStore.removePatterns(remove);
}
