// Per-channel instrument settings (Bloom — see docs/instrument.md). Saved in
// the .alp as ChannelSettings.instrument (project.rs's InstrumentXml).
//
// When `enabled` is false the channel behaves exactly as before instruments
// existed (fixed engine envelope, step hits ring out). When true, the amp
// envelope/filter below shape every voice; the sound comes from the SFZ
// preset at `presetPath`, or from the channel's own sample/folder when no
// preset is loaded.

export type FilterType = 'lowpass' | 'highpass' | 'bandpass';
export const FILTER_TYPES: FilterType[] = ['lowpass', 'highpass', 'bandpass'];

export const MACRO_COUNT = 8;
export const CUTOFF_MIN = 20;
export const CUTOFF_MAX = 20000;

export interface InstrumentSettings {
  enabled: boolean;
  presetPath: string;      // data-relative .sfz path; '' = use the channel's sample/folder
  attack: number;          // seconds
  decay: number;           // seconds
  sustain: number;         // 0..1 of peak
  release: number;         // seconds
  filterType: FilterType;
  cutoff: number;          // Hz, CUTOFF_MIN..CUTOFF_MAX (max + no env depth = no filter)
  resonance: number;       // dB, 0..24 (SFZ's unit)
  filterEnvDepth: number;  // cents, -9600..9600
  filterEnvAttack: number; // seconds
  filterEnvDecay: number;  // seconds (decays back to the base cutoff)
  tune: number;            // semitones, -24..24
  macros: number[];        // MACRO_COUNT values, 0..1 — mapped by the preset's label_cc/…_oncc opcodes
}

// Must match project.rs's `impl Default for InstrumentXml`. The envelope
// defaults equal the engine's fixed piano-roll envelope, so enabling an
// instrument on a plain sample channel changes nothing until a knob moves.
export function defaultInstrument(): InstrumentSettings {
  return {
    enabled: false,
    presetPath: '',
    attack: 0.001,
    decay: 0.05,
    sustain: 0.7,
    release: 0.05,
    filterType: 'lowpass',
    cutoff: CUTOFF_MAX,
    resonance: 0,
    filterEnvDepth: 0,
    filterEnvAttack: 0.001,
    filterEnvDecay: 0.3,
    tune: 0,
    macros: Array(MACRO_COUNT).fill(0.5),
  };
}

const num = (v: unknown, fallback: number) => (typeof v === 'number' && Number.isFinite(v) ? v : fallback);

// Import boundary: fills anything missing (older saves/history snapshots),
// and unwraps the XML wrapper shape `macros: { value: [...] }`.
export function normalizeInstrument(raw: unknown): InstrumentSettings {
  const d = defaultInstrument();
  if (!raw || typeof raw !== 'object') return d;
  const r = raw as Record<string, unknown>;
  const rawMacros = Array.isArray(r.macros) ? r.macros : (r.macros as { value?: unknown } | undefined)?.value;
  const macros = Array.isArray(rawMacros) ? rawMacros : [];
  return {
    enabled: r.enabled === true,
    presetPath: typeof r.presetPath === 'string' ? r.presetPath : '',
    attack: num(r.attack, d.attack),
    decay: num(r.decay, d.decay),
    sustain: num(r.sustain, d.sustain),
    release: num(r.release, d.release),
    filterType: FILTER_TYPES.includes(r.filterType as FilterType) ? (r.filterType as FilterType) : d.filterType,
    cutoff: num(r.cutoff, d.cutoff),
    resonance: num(r.resonance, d.resonance),
    filterEnvDepth: num(r.filterEnvDepth, d.filterEnvDepth),
    filterEnvAttack: num(r.filterEnvAttack, d.filterEnvAttack),
    filterEnvDecay: num(r.filterEnvDecay, d.filterEnvDecay),
    tune: num(r.tune, d.tune),
    macros: Array.from({ length: MACRO_COUNT }, (_, i) => num(macros[i], 0.5)),
  };
}

// The shape project.rs's InstrumentXml deserializes (macros wrapped so
// quick-xml writes one <value> tag per entry).
export interface InstrumentExport extends Omit<InstrumentSettings, 'macros'> {
  macros: { value: number[] };
}

export function exportInstrument(inst: InstrumentSettings): InstrumentExport {
  return { ...inst, macros: { value: [...inst.macros] } };
}
