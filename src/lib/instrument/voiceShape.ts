import { CUTOFF_MAX, CUTOFF_MIN, MACRO_COUNT, type FilterType, type InstrumentSettings } from './types';
import type { SfzInstrument } from './sfz';

// Everything the engine needs to shape one instrument voice, resolved from
// the channel's settings plus the macro knobs (via the preset's CC
// modulation opcodes). Per-region gain/pan/tune and per-note properties are
// applied on top by the engine.
export interface VoiceShape {
  attack: number;
  decay: number;
  sustain: number;  // 0..1
  release: number;
  filter: {
    type: FilterType;
    cutoff: number;    // Hz
    resonance: number; // dB
    envDepth: number;  // cents
    envAttack: number;
    envDecay: number;
  } | null;           // null = no filter node at all
  gainDb: number;
  pan: number;        // -1..1 offset
  pitchCents: number;
}

// Macro-modulatable parameters. Each maps an SFZ `<param>_onccN` (also
// `<param>_ccN` / `<param>ccN`) opcode to the field it offsets.
const MOD_TARGETS = [
  'cutoff', 'resonance', 'ampeg_attack', 'ampeg_decay', 'ampeg_sustain', 'ampeg_release',
  'fileg_depth', 'fileg_attack', 'fileg_decay', 'volume', 'gain', 'pan', 'pitch', 'tune',
] as const;
type ModTarget = typeof MOD_TARGETS[number];

export interface MacroMod { target: ModTarget; amount: number }

const MOD_RE = new RegExp(`^(${MOD_TARGETS.join('|')})_?(?:on)?cc(\\d+)$`);

// What each macro slot does: slot i is the preset's i-th labeled CC
// (sfz.ts sorts controls by CC number), modulations read from the first
// region's effective opcodes.
const modsCache = new WeakMap<SfzInstrument, MacroMod[][]>();

export function macroMods(preset: SfzInstrument | null): MacroMod[][] {
  const cached = preset && modsCache.get(preset);
  if (cached) return cached;
  const slots: MacroMod[][] = Array.from({ length: MACRO_COUNT }, () => []);
  if (!preset || preset.regions.length === 0) return slots;
  const ops = preset.regions[0].opcodes;
  const slotOfCc = new Map(preset.controls.slice(0, MACRO_COUNT).map((c, i) => [c.cc, i]));
  for (const [k, v] of Object.entries(ops)) {
    const m = MOD_RE.exec(k);
    if (!m) continue;
    const slot = slotOfCc.get(parseInt(m[2], 10));
    const amount = parseFloat(v);
    if (slot === undefined || !Number.isFinite(amount)) continue;
    slots[slot].push({ target: m[1] as ModTarget, amount });
  }
  modsCache.set(preset, slots);
  return slots;
}

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

function filterTypeFromSfz(v: string | undefined): FilterType {
  if (v?.startsWith('hpf')) return 'highpass';
  if (v?.startsWith('bpf')) return 'bandpass';
  return 'lowpass';
}

const opNum = (ops: Record<string, string>, k: string, fallback: number) => {
  const n = parseFloat(ops[k] ?? '');
  return Number.isFinite(n) ? n : fallback;
};

// Channel settings a freshly loaded preset starts from — read from its
// first region (envelope/filter are channel-wide in Bloom, not per region).
export function presetDefaults(preset: SfzInstrument): Partial<InstrumentSettings> {
  const ops = preset.regions[0]?.opcodes ?? {};
  return {
    attack: Math.max(0.001, opNum(ops, 'ampeg_attack', 0)),
    decay: opNum(ops, 'ampeg_decay', 0),
    sustain: clamp(opNum(ops, 'ampeg_sustain', 100) / 100, 0, 1),
    release: Math.max(0.01, opNum(ops, 'ampeg_release', 0.001)),
    filterType: filterTypeFromSfz(ops.fil_type),
    cutoff: clamp(opNum(ops, 'cutoff', CUTOFF_MAX), CUTOFF_MIN, CUTOFF_MAX),
    resonance: clamp(opNum(ops, 'resonance', 0), 0, 24),
    filterEnvDepth: opNum(ops, 'fileg_depth', 0),
    filterEnvAttack: Math.max(0.001, opNum(ops, 'fileg_attack', 0)),
    filterEnvDecay: opNum(ops, 'fileg_decay', 0.3),
    tune: 0,
    macros: Array.from({ length: MACRO_COUNT }, (_, i) => preset.controls[i]?.value ?? 0),
  };
}

export function instrumentShape(s: InstrumentSettings, preset: SfzInstrument | null, mods = macroMods(preset)): VoiceShape {
  let { attack, decay, sustain, release, cutoff, resonance } = s;
  let envDepth = s.filterEnvDepth, envAttack = s.filterEnvAttack, envDecay = s.filterEnvDecay;
  let gainDb = 0, pan = 0, pitchCents = s.tune * 100, cutoffCents = 0;
  mods.forEach((slot, i) => {
    const v = s.macros[i] ?? 0;
    for (const { target, amount } of slot) {
      const d = amount * v;
      switch (target) {
        case 'cutoff': cutoffCents += d; break;
        case 'resonance': resonance += d; break;
        case 'ampeg_attack': attack += d; break;
        case 'ampeg_decay': decay += d; break;
        case 'ampeg_sustain': sustain += d / 100; break;
        case 'ampeg_release': release += d; break;
        case 'fileg_depth': envDepth += d; break;
        case 'fileg_attack': envAttack += d; break;
        case 'fileg_decay': envDecay += d; break;
        case 'volume': case 'gain': gainDb += d; break;
        case 'pan': pan += d / 100; break;
        case 'pitch': case 'tune': pitchCents += d; break;
      }
    }
  });
  cutoff = clamp(cutoff * 2 ** (cutoffCents / 1200), CUTOFF_MIN, CUTOFF_MAX);
  const needsFilter = cutoff < CUTOFF_MAX || envDepth !== 0 || resonance > 0 || s.filterType !== 'lowpass';
  return {
    attack: Math.max(0.001, attack),
    decay: Math.max(0, decay),
    sustain: clamp(sustain, 0, 1),
    release: Math.max(0.005, release),
    filter: needsFilter ? {
      type: s.filterType, cutoff, resonance: clamp(resonance, 0, 40), envDepth,
      envAttack: Math.max(0.001, envAttack), envDecay: Math.max(0.001, envDecay),
    } : null,
    gainDb,
    pan: clamp(pan, -1, 1),
    pitchCents,
  };
}
