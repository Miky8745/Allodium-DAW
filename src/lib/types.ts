export interface FileNode {
  name: string;
  is_dir: boolean;
  children: FileNode[];
}

export interface FlatNode {
  node: FileNode;
  depth: number;
  expanded: boolean;
  id: string;
}

export interface MenuItem {
  label: string;
  action: () => void;
}

export interface Note {
  id: number;
  pitch: number;    // MIDI number, octave convention 60 = C5
  start: number;    // step index within the pattern (0..patternLength-1); may be
                     // fractional when moved/resized with Shift held in the Piano
                     // Roll (free/unsnapped editing)
  length: number;   // in 16th-note steps, >= ~0.05; may be fractional under free editing
  velocity: number; // 0..1
  // Note properties (Piano Roll's properties panel, see noteProps.ts for
  // defaults/ranges). Older saves lack these — normalizeNote() fills them in.
  pan: number;       // 0..1, 0.5 = center; offsets the channel's own pan
  release: number;   // 0..1, 0.5 = the engine's default release time
  modX: number;      // 0..1, lowpass cutoff (1 = fully open, no filter)
  modY: number;      // 0..1, filter resonance (0 = none)
  finePitch: number; // cents, -1200..1200
  color: number;     // index into noteColorStore.palette (0..14)
  // Slide (pitch-bend) note: never sounds itself — glides every same-color
  // note of the same channel that's playing when it starts to its pitch,
  // over its own length. See slideVoices.ts.
  slide: boolean;
}

export interface ChannelData {
  id: number;
  samplePath: string | null;
  sampleFolder: string | null; // relative path to a multisample instrument folder; mutually exclusive with samplePath
  muted: boolean;
  soloed: boolean; // Ctrl+click the mute dot in ChannelRow to toggle (channelStore.toggleSolo). Single-solo: at most one channel soloed at a time. Channel-Rack-only — unrelated to the Mixer's own separate soloed field.
  pan: number;
  volume: number;
  mixerTrack: number;
  steps: boolean[];
  notes: Note[];
  name: string | null;  // user-set override (right-click > Rename); null = derive from sample, see formatChannelLabel
  color: string | null; // hex "#rrggbb" applied to the sample-name box only (right-click > Recolor/Recolor randomly); null = default styling
  midiOutPort: string | null; // MIDI output port key (right-click > MIDI out…); only piano-roll notes are sent. null = none
  midiOutChannel: number;     // 1-16
}

// Payload carried by FileExplorer's `filedrop` custom event. `x`/`y` are the
// client coordinates at drop time (FileExplorer already tracks these during
// its own drag-only pointermove listener) — carried here so consumers that
// need sub-row drop precision (Playlist) don't have to run a second,
// always-on document-wide pointermove listener of their own just to know
// where the cursor was.
export type FileDropDetail =
  | { kind: 'file'; path: string; x: number; y: number }
  | { kind: 'folder'; path: string; files: string[]; x: number; y: number };

// Payload carried by FileExplorer's `filedragover` custom event, dispatched
// on the current drop target on every pointermove tick while a file/folder
// drag is over it (unlike `filedragenter`/`filedragleave`, which only fire
// once when the hovered target itself changes). This is what lets a drop
// zone track live cursor position for something like an insertion-line
// indicator (see ChannelRack.svelte) without standing up a second
// document-wide pointermove listener of its own — FileExplorer already runs
// one for its own drag tracking, so this just rides on it.
export type FileDragOverDetail = { x: number; y: number };

export interface PatternData {
  id: number;
  name: string;
  color: string;
  lengthBeats: number; // decimals allowed (e.g. for unquantized vocal/mp3 clips)
}

// A placed pattern instance or audio clip on a Playlist track. `kind`
// discriminates which of patternId/samplePath+lengthBeats is meaningful —
// the unused side always carries a concrete sentinel (0 / '' / 0) rather
// than being optional, so the Rust XML side never has to round-trip an
// Option<T> for these fields (see project.rs's PlacementXml).
export interface Placement {
  id: number;
  kind: 'pattern' | 'clip';
  trackId: number;
  startBeat: number;
  patternId: number;    // real PatternData id when kind==='pattern'; 0 (never a real id) when kind==='clip'
  samplePath: string;   // real data-relative path when kind==='clip'; '' when kind==='pattern'
  lengthBeats: number;  // real clip length when kind==='clip'; 0 when kind==='pattern' (pattern length always comes from PatternData.lengthBeats instead)
}
