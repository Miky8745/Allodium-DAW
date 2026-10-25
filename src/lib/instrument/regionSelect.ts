import type { SfzInstrument, SfzRegion } from './sfz';

// Round-robin counters (keyed by preset path + group index) and the active
// keyswitch (keyed by player — a channel id — + preset path). Module state:
// playback bookkeeping, never saved.
const seqCounters = new Map<string, number>();
const activeSwitch = new Map<string, number>();

const toVel127 = (v01: number) => Math.max(1, Math.min(127, Math.round(v01 * 127)));

// Every region that should sound for one note: all regions whose key and
// velocity ranges match (SFZ layers them), filtered by keyswitch, round
// robin (seq_length/seq_position, one counter per <group>), random selection
// (lorand/hirand, one random draw per note) and velocity crossfades. A note
// inside the keyswitch range only switches articulation and plays nothing.
// `player` scopes keyswitch state (one per channel).
export function selectRegions(inst: SfzInstrument, pitch: number, velocity01: number, player = 0): SfzRegion[] {
  const vel = toVel127(velocity01);
  const key = Math.round(pitch);
  const ks = inst.keyswitches;
  const swKey = `${player}#${inst.path}`;
  if (ks && key >= ks.lo && key <= ks.hi) {
    activeSwitch.set(swKey, key);
    return [];
  }
  const sw = ks ? activeSwitch.get(swKey) ?? ks.initial : null;
  const rand = Math.random();
  const matching = inst.regions.filter(r =>
    key >= r.lokey && key <= r.hikey && vel >= r.lovel && vel <= r.hivel && rand >= r.lorand && rand < r.hirand
    && (r.swLast === null || r.swLast === sw) && velocityGain(r, velocity01) > 0);

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

// Velocity crossfade + amp_veltrack, as a multiplier on top of the engine's
// own `volume × velocity` (so it divides that velocity back out where the
// region tracks velocity less than fully).
export function velocityGain(r: SfzRegion, velocity01: number): number {
  const vel = toVel127(velocity01);
  let g = 1;
  // A zero-width range (e.g. the 127/127 default) means no fade at all.
  if (r.xfin && r.xfin[1] > r.xfin[0]) {
    const [lo, hi] = r.xfin;
    g *= vel <= lo ? 0 : vel >= hi ? 1 : Math.sqrt((vel - lo) / (hi - lo));
  }
  if (r.xfout && r.xfout[1] > r.xfout[0]) {
    const [lo, hi] = r.xfout;
    g *= vel >= hi ? 0 : vel <= lo ? 1 : Math.sqrt((hi - vel) / (hi - lo));
  }
  if (r.ampVeltrack < 1) {
    const v = Math.max(1 / 127, velocity01);
    g *= (1 - r.ampVeltrack + r.ampVeltrack * v) / v;
  }
  return g;
}

// Every sample a preset can play — for preloading.
export function presetSamples(inst: SfzInstrument): string[] {
  return [...new Set(inst.regions.map(r => r.sample))];
}
