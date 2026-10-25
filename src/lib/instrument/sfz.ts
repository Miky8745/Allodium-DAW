// Minimal SFZ parser — the subset Bloom plays (see docs/instrument.md for
// the supported opcode list). Unknown opcodes are kept in `opcodes` but
// otherwise ignored, so a richer SFZ still loads and plays its mapping.

export interface SfzRegion {
  sample: string;       // data-relative path, already resolved against the .sfz's folder
  lokey: number;        // MIDI numbers (SFZ convention: c4 = 60 = Allodium's C5)
  hikey: number;
  keycenter: number;
  lovel: number;        // 1..127
  hivel: number;
  tune: number;         // cents (tune + transpose*100)
  volume: number;       // dB
  pan: number;          // -100..100
  oneShot: boolean;     // loop_mode=one_shot: ignores note length, rings out
  loop: { start: number; end: number } | 'whole' | null; // sample frames (source rate); 'whole' = loop the entire file
  offset: number;       // start offset, sample frames (source rate)
  group: number;        // index of the <group> it belongs to (round-robin counter key)
  seqLength: number;
  seqPosition: number;
  lorand: number;
  hirand: number;
  pitchKeytrack: number; // cents per key (100 = normal; 0 = fixed pitch, e.g. drums)
  ampVeltrack: number;   // 0..1 — how much velocity affects loudness (1 = fully)
  // Velocity crossfades (xfin_/xfout_lovel/hivel), 1..127; null = none.
  xfin: [number, number] | null;
  xfout: [number, number] | null;
  // Keyswitch: the region only plays while the last switch key pressed
  // (within the group's sw_lokey..sw_hikey) is `swLast`; null = always.
  swLast: number | null;
  opcodes: Record<string, string>; // effective (inherited) opcodes, for envelope/filter/macro lookups
}

export interface SfzControl {
  cc: number;
  label: string;
  value: number; // default 0..1 (set_ccN / 127)
}

export interface SfzKeyswitches {
  lo: number;      // keys lo..hi are switches, not notes
  hi: number;
  initial: number; // sw_default
  labels: Map<number, string>;
}

export interface SfzInstrument {
  path: string;
  name: string;
  regions: SfzRegion[];
  keyswitches: SfzKeyswitches | null;
  controls: SfzControl[]; // labeled CCs, in CC order — these become the macro knobs
}

const NOTE_OFFSETS: Record<string, number> = { c: 0, d: 2, e: 4, f: 5, g: 7, a: 9, b: 11 };

// "60", "c4", "c#4", "db4", "C-1" → MIDI number (c4 = 60). NaN if unparsable.
export function parseSfzKey(raw: string): number {
  const v = raw.trim();
  if (/^-?\d+$/.test(v)) return parseInt(v, 10);
  const m = /^([a-gA-G])([#b]?)(-?\d+)$/.exec(v);
  if (!m) return NaN;
  const acc = m[2] === '#' ? 1 : m[2] === 'b' ? -1 : 0;
  return NOTE_OFFSETS[m[1].toLowerCase()] + acc + (parseInt(m[3], 10) + 1) * 12;
}

// Joins and normalizes "a/b" + "../c/d.wav" → "a/c/d.wav". Returns null if
// the result would escape the data root (Rust rejects ".." anyway).
export function resolvePath(baseDir: string, rel: string): string | null {
  const parts = (baseDir ? `${baseDir}/${rel}` : rel).replace(/\\/g, '/').split('/');
  const out: string[] = [];
  for (const p of parts) {
    if (p === '' || p === '.') continue;
    if (p === '..') {
      if (out.length === 0) return null;
      out.pop();
    } else out.push(p);
  }
  return out.join('/');
}

function stripComments(text: string): string {
  return text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '');
}

type Header = 'control' | 'global' | 'master' | 'group' | 'region' | 'other';

interface Token { kind: 'header'; name: string }
interface OpToken { kind: 'op'; key: string; value: string }

// Tokenizes line by line. An opcode's value runs until the next opcode or
// header on the same line, which is what lets `sample=` contain spaces.
function tokenize(text: string): (Token | OpToken)[] {
  const tokens: (Token | OpToken)[] = [];
  const defines = new Map<string, string>();
  for (let line of stripComments(text).split(/\r?\n/)) {
    const def = /^\s*#define\s+(\$\w+)\s+(.*?)\s*$/.exec(line);
    if (def) { defines.set(def[1], def[2]); continue; }
    if (/^\s*#/.test(line)) continue; // #include etc. — unsupported
    for (const [k, v] of defines) line = line.split(k).join(v);
    const re = /<(\w+)>|([A-Za-z0-9_]+)=/g;
    const matches = [...line.matchAll(re)];
    matches.forEach((m, i) => {
      if (m[1]) { tokens.push({ kind: 'header', name: m[1].toLowerCase() }); return; }
      const start = m.index! + m[0].length;
      const end = i + 1 < matches.length ? matches[i + 1].index! : line.length;
      tokens.push({ kind: 'op', key: m[2].toLowerCase(), value: line.slice(start, end).trim() });
    });
  }
  return tokens;
}

const toNum = (v: string | undefined, fallback: number) => {
  if (v === undefined) return fallback;
  const n = parseFloat(v);
  return Number.isFinite(n) ? n : fallback;
};

const toKey = (v: string | undefined, fallback: number) => {
  if (v === undefined) return fallback;
  const n = parseSfzKey(v);
  return Number.isFinite(n) ? n : fallback;
};

function buildRegion(ops: Record<string, string>, baseDir: string, group: number): SfzRegion | null {
  if (!ops.sample || (ops.trigger && ops.trigger !== 'attack')) return null;
  // default_path is relative to the .sfz's own folder.
  const sample = resolvePath(baseDir, (ops.default_path ?? '') + ops.sample);
  if (!sample) return null;
  const key = ops.key !== undefined ? toKey(ops.key, NaN) : NaN;
  const lokey = Number.isFinite(key) ? key : toKey(ops.lokey, 0);
  const hikey = Number.isFinite(key) ? key : toKey(ops.hikey, 127);
  const keycenter = toKey(ops.pitch_keycenter, Number.isFinite(key) ? key : 60);
  const loopMode = ops.loop_mode ?? '';
  const looping = loopMode === 'loop_continuous' || loopMode === 'loop_sustain';
  const loopStart = toNum(ops.loop_start ?? ops.loopstart, NaN);
  const loopEnd = toNum(ops.loop_end ?? ops.loopend, NaN);
  return {
    sample,
    lokey, hikey, keycenter,
    lovel: toNum(ops.lovel, 1),
    hivel: toNum(ops.hivel, 127),
    tune: toNum(ops.tune, 0) + toNum(ops.transpose, 0) * 100,
    volume: toNum(ops.volume, 0),
    pan: toNum(ops.pan, 0),
    oneShot: loopMode === 'one_shot',
    loop: !looping ? null : Number.isFinite(loopStart) && Number.isFinite(loopEnd) && loopEnd > loopStart ? { start: loopStart, end: loopEnd } : 'whole',
    offset: toNum(ops.offset, 0),
    group,
    seqLength: Math.max(1, toNum(ops.seq_length, 1)),
    seqPosition: Math.max(1, toNum(ops.seq_position, 1)),
    lorand: toNum(ops.lorand, 0),
    hirand: toNum(ops.hirand, 1),
    pitchKeytrack: toNum(ops.pitch_keytrack, 100),
    ampVeltrack: Math.max(0, Math.min(1, toNum(ops.amp_veltrack, 100) / 100)),
    xfin: ops.xfin_hivel !== undefined ? [toNum(ops.xfin_lovel, 0), toNum(ops.xfin_hivel, 0)] : null,
    xfout: ops.xfout_lovel !== undefined ? [toNum(ops.xfout_lovel, 127), toNum(ops.xfout_hivel, 127)] : null,
    swLast: ops.sw_last !== undefined ? toKey(ops.sw_last, NaN) : null,
    opcodes: ops,
  };
}

export function presetNameFromPath(path: string): string {
  return (path.split('/').pop() ?? path).replace(/\.sfz$/i, '');
}

// `path` is the .sfz's data-relative path; sample paths resolve against its folder.
export function parseSfz(text: string, path: string): SfzInstrument {
  const baseDir = path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '';
  let control: Record<string, string> = {};
  let global: Record<string, string> = {};
  let master: Record<string, string> = {};
  let group: Record<string, string> = {};
  let current: Record<string, string> | null = null;
  let header: Header = 'other';
  let groupIndex = 0;
  const regions: SfzRegion[] = [];

  const flushRegion = () => {
    if (header !== 'region' || !current) return;
    const r = buildRegion({ ...control, ...global, ...master, ...group, ...current }, baseDir, groupIndex);
    if (r) regions.push(r);
  };

  for (const t of tokenize(text)) {
    if (t.kind === 'header') {
      flushRegion();
      header = (['control', 'global', 'master', 'group', 'region'] as const).find(h => h === t.name) ?? 'other';
      current = {};
      if (header === 'control') control = current;
      else if (header === 'global') { global = current; master = {}; group = {}; }
      else if (header === 'master') { master = current; group = {}; }
      else if (header === 'group') { group = current; groupIndex++; }
      continue;
    }
    if (current) current[t.key] = t.value;
  }
  flushRegion();

  // Labeled CCs (label_ccN) become macros; set_ccN gives their default.
  const controls: SfzControl[] = [];
  for (const [k, v] of Object.entries(control)) {
    const m = /^label_cc(\d+)$/.exec(k);
    if (!m) continue;
    const cc = parseInt(m[1], 10);
    controls.push({ cc, label: v, value: Math.max(0, Math.min(1, toNum(control[`set_cc${cc}`], 0) / 127)) });
  }
  controls.sort((a, b) => a.cc - b.cc);

  return { path, name: presetNameFromPath(path), regions, controls, keyswitches: collectKeyswitches(regions) };
}

// The switch-key range/default/labels, from whichever regions declare them.
function collectKeyswitches(regions: SfzRegion[]): SfzKeyswitches | null {
  let ks: SfzKeyswitches | null = null;
  for (const r of regions) {
    const o = r.opcodes;
    if (o.sw_lokey === undefined || o.sw_hikey === undefined) continue;
    const lo = toKey(o.sw_lokey, NaN), hi = toKey(o.sw_hikey, NaN);
    if (!Number.isFinite(lo) || !Number.isFinite(hi)) continue;
    ks ??= { lo, hi, initial: toKey(o.sw_default, lo), labels: new Map() };
    ks.lo = Math.min(ks.lo, lo);
    ks.hi = Math.max(ks.hi, hi);
    if (r.swLast !== null && o.sw_label && !ks.labels.has(r.swLast)) ks.labels.set(r.swLast, o.sw_label);
  }
  return ks;
}
