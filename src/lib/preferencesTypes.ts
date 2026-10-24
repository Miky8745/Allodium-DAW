// Mirrors src-tauri/src/preferences.rs field-for-field (including the
// quick-xml per-item wrapper objects), same convention as
// projectSerializer.ts's ProjectFileData. "Unset" values are sentinels, not
// null (see the Rust file's header comment for why).

export type LatencyHint = 'interactive' | 'balanced' | 'playback';
export type MonitorSource = 'app' | 'system';
export type VelocityCurve = 'linear' | 'soft' | 'hard' | 'custom';

export interface GeneralPrefs {
  guiScale: number;
  latencyHint: LatencyHint;
  sampleRate: number;      // 0 = auto
  lookaheadMs: number;
  tickMs: number;
  autosaveEnabled: boolean;
  autosaveDelayMs: number;
  undoLimit: number;
  reopenLastProject: boolean;
  lastProject: string;     // '' = none
  defaultBpm: number;
  defaultBeatsPerBar: number;
  previewVolume: number;   // 0..1
  previewLengthMs: number;
  qwertyVelocity: number;  // 1..127
  monitorSource: MonitorSource; // Toolbar audio monitor input
}

export interface MidiInputPrefs {
  name: string;            // port key (see midi.rs's port_key)
  enabled: boolean;
  transpose: number;       // semitones
  channel: number;         // 0 = omni, else 1-16
  noteLow: number;
  noteHigh: number;
  velCurve: VelocityCurve;
  velExponent: number;     // used when velCurve === 'custom'
  velOffset: number;
  velMin: number;
  velMax: number;
  fixedVelocity: number;   // 0 = off, else 1-127
  bendRange: number;       // semitones at full pitch-bend deflection
}

export interface MidiOutputPrefs {
  name: string;
  enabled: boolean;
  latencyMs: number;
  transpose: number;
  velScale: number;
  allNotesOffOnStop: boolean;
}

export interface Preferences {
  general: GeneralPrefs;
  midiInputs: { input: MidiInputPrefs[] };
  midiOutputs: { output: MidiOutputPrefs[] };
}

export function defaultGeneralPrefs(): GeneralPrefs {
  return {
    guiScale: 1,
    latencyHint: 'interactive',
    sampleRate: 0,
    lookaheadMs: 100,
    tickMs: 25,
    autosaveEnabled: true,
    autosaveDelayMs: 1000,
    undoLimit: 250,
    reopenLastProject: false,
    lastProject: '',
    defaultBpm: 120,
    defaultBeatsPerBar: 4,
    previewVolume: 0.8,
    previewLengthMs: 150,
    qwertyVelocity: 100,
    monitorSource: 'app',
  };
}

export function defaultInputPrefs(name: string): MidiInputPrefs {
  return {
    name, enabled: false, transpose: 0, channel: 0, noteLow: 0, noteHigh: 127,
    velCurve: 'linear', velExponent: 1, velOffset: 0, velMin: 1, velMax: 127, fixedVelocity: 0,
    bendRange: 2,
  };
}

export function defaultOutputPrefs(name: string): MidiOutputPrefs {
  return { name, enabled: false, latencyMs: 0, transpose: 0, velScale: 1, allNotesOffOnStop: true };
}
