import type { SfzInstrument, SfzRegion } from './sfz';

// Round-robin counters, keyed by preset path + group index. Module state —
// it's playback bookkeeping, never saved.
const seqCounters = new Map<string, number>();

// Every region that should sound for one note: all regions whose key and
// velocity ranges match (SFZ layers them), filtered by round robin
// (seq_length/seq_position, one counter per <group>) and random selection
// (lorand/hirand, one random draw per note).
export function selectRegions(inst: SfzInstrument, pitch: number, velocity01: number): SfzRegion[] {
  const vel = Math.max(1, Math.min(127, Math.round(velocity01 * 127)));
  const key = Math.round(pitch);
  const rand = Math.random();
  const matching = inst.regions.filter(r =>
    key >= r.lokey && key <= r.hikey && vel >= r.lovel && vel <= r.hivel && rand >= r.lorand && rand < r.hirand);

  const counterFor = new Map<number, number>();
  for (const r of matching) {
    if (r.seqLength <= 1 || counterFor.has(r.group)) continue;
    const k = `${inst.path}#${r.group}`;
    const n = seqCounters.get(k) ?? 0;
    seqCounters.set(k, n + 1);
    counterFor.set(r.group, n);
  }
  return matching.filter(r => r.seqLength <= 1 || (counterFor.get(r.group)! % r.seqLength) + 1 === r.seqPosition);
}

// Every sample a preset can play — for preloading.
export function presetSamples(inst: SfzInstrument): string[] {
  return [...new Set(inst.regions.map(r => r.sample))];
}
