import type { ChannelData, Note } from './types';
import { normalizeNote } from './noteProps';
import { profiler } from './debug/profiler';

let nextId = 1;

const DEFAULT_STEP_COUNT = 16;

function makeChannel(id: number, stepCount: number): ChannelData {
  return {
    id,
    samplePath: null,
    sampleFolder: null,
    muted: false,
    soloed: false,
    pan: 0.5,
    volume: 0.8,
    mixerTrack: 0,
    steps: Array(stepCount).fill(false) as boolean[],
    notes: [],
    name: null,
    color: null,
    midiOutPort: null,
    midiOutChannel: 1,
  };
}

interface PatternContent {
  steps: boolean[];
  notes: Note[];
}

export interface ChannelSettingsExport {
  id: number;
  samplePath: string | null;
  sampleFolder: string | null;
  muted: boolean;
  soloed: boolean;
  pan: number;
  volume: number;
  mixerTrack: number;
  name: string | null;
  color: string | null;
  midiOutPort: string | null;
  midiOutChannel: number;
}

export interface PatternContentEntry {
  patternId: number;
  channelId: number;
  steps: boolean[];
  notes: Note[];
}

export interface ChannelStoreExport {
  channels: ChannelSettingsExport[];
  content: PatternContentEntry[];
  beatsPerBar: number;
  patternLength: number;
  stepCount: number;
}

class ChannelStore {
  channels = $state<ChannelData[]>([makeChannel(0, DEFAULT_STEP_COUNT)]);
  selectedChannelId = $state<number | null>(null);

  #beatsPerBar = $state(4);
  #patternLength = $state(16);

  // How many step-sequencer buttons each channel's `steps` array has, shared
  // by every channel and every pattern (same "one global number" convention
  // as beatsPerBar/patternLength — only the per-pattern *content* is
  // per-pattern-scoped, not the grid's own size). Grows via addSteps() below
  // — dragging the Channel Rack window wider (ChannelRack.svelte) or the
  // rack's right-click "Add steps…" menu — and never shrinks.
  #stepCount = $state(DEFAULT_STEP_COUNT);

  get stepCount(): number {
    return this.#stepCount;
  }

  // Step/note content is scoped per pattern (keyed by pattern id, then channel
  // id); channel identity and settings (sample, mute, pan, volume, mixer
  // track) are shared across all patterns. patternStore calls switchPattern()
  // whenever the selected pattern changes.
  #patternContent = $state(new Map<number, Map<number, PatternContent>>());

  // One bar is always 4 steps per beat (16th notes), so a bar in 4/4 is 16
  // steps, 3/4 is 12, 2/4 is 8, etc. patternLength is kept snapped to a whole
  // multiple of this so the pattern never ends mid-bar.
  get barLength(): number {
    return this.#beatsPerBar * 4;
  }

  get beatsPerBar(): number {
    return this.#beatsPerBar;
  }

  set beatsPerBar(value: number) {
    const bars = Math.max(1, Math.round(this.#patternLength / this.barLength));
    this.#beatsPerBar = Math.max(1, Math.min(16, Math.round(value)));
    this.#patternLength = bars * this.barLength;
  }

  get patternLength(): number {
    return this.#patternLength;
  }

  set patternLength(value: number) {
    const bars = Math.max(1, Math.round(value / this.barLength));
    this.#patternLength = bars * this.barLength;
  }

  // Grows the step sequencer by `beats` groups of 4 steps (the fixed
  // steps-per-beat convention used throughout — see "Step convention" in
  // CLAUDE.md), appending `false` to every channel's `steps` array across
  // every pattern (the live channels plus every stashed pattern in
  // #patternContent) so the rack stays the same width regardless of which
  // pattern is open. Additive only — there's no corresponding "remove
  // steps"; a non-positive/invalid `beats` is a no-op.
  //
  // Also raises patternLength (snapped up to a whole bar, same rule as its
  // own setter) if stepCount would otherwise grow past it: patternLength is
  // what bounds activeLength's bar-occupancy scan, which is in turn what
  // every channel's own loop length (channelLoopLength) is computed
  // against — a step beyond patternLength is never scanned into either, so
  // without this a newly added step could be painted on but would never
  // actually play. Unlike the Piano Roll's own auto-extend (which only
  // grows patternLength once a note reaches the last bar), this grows it
  // right away so every added step is playable immediately, not just once
  // something's drawn in it.
  addSteps(beats: number) {
    const add = Math.max(0, Math.round(beats)) * 4;
    if (add <= 0) return;
    this.#stepCount += add;
    if (this.#patternLength < this.#stepCount) {
      const bars = Math.ceil(this.#stepCount / this.barLength);
      this.#patternLength = bars * this.barLength;
    }
    const grow = (steps: boolean[]) => {
      while (steps.length < this.#stepCount) steps.push(false);
    };
    for (const ch of this.channels) grow(ch.steps);
    for (const chMap of this.#patternContent.values()) {
      for (const content of chMap.values()) grow(content.steps);
    }
  }

  // The inverse of addSteps(): shrinks stepCount back down to drop unused
  // trailing steps, scanning every channel's steps array across *every*
  // pattern (the live channels plus every stashed pattern in
  // #patternContent — a pattern never visited yet has no stash entry at all
  // and so can't be holding any content either, correctly contributing
  // nothing). Only ever removes steps from the end, never the middle: finds
  // the single highest step index that's active in any channel in any
  // pattern, and trims everything strictly after it (rounded back up to a
  // whole beat, matching addSteps()'s own 4-step granularity, and floored
  // at one beat so the rack never trims itself down to nothing). A step
  // that happens to be unused in every pattern but sits *before* that
  // highest-used index is left alone — it's "in the middle" as far as the
  // array is concerned, even though no pattern actually uses it.
  trimSteps() {
    const lastActiveIndex = (steps: boolean[]): number => {
      for (let i = steps.length - 1; i >= 0; i--) {
        if (steps[i]) return i;
      }
      return -1;
    };
    let maxUsed = -1;
    for (const ch of this.channels) maxUsed = Math.max(maxUsed, lastActiveIndex(ch.steps));
    for (const chMap of this.#patternContent.values()) {
      for (const content of chMap.values()) maxUsed = Math.max(maxUsed, lastActiveIndex(content.steps));
    }
    const target = Math.max(4, Math.ceil((maxUsed + 1) / 4) * 4);
    if (target >= this.#stepCount) return; // nothing trailing to trim
    this.#stepCount = target;
    const shrink = (steps: boolean[]) => {
      if (steps.length > target) steps.length = target;
    };
    for (const ch of this.channels) shrink(ch.steps);
    for (const chMap of this.#patternContent.values()) {
      for (const content of chMap.values()) shrink(content.steps);
    }
  }

  // The actual content-driven playback loop bound, as opposed to
  // patternLength (the editable canvas size, grown ahead of content by
  // PianoRoll's auto-extend effect to give the user room to keep drawing). A
  // bar only counts toward this if *some* channel has content there — so a
  // freshly auto-extended trailing bar that's still empty everywhere doesn't
  // count until something is actually drawn into it. This is what lets a
  // channel with genuinely short content (e.g. a 1-bar kick loop) keep
  // independently looping within a longer pattern, while a channel whose
  // content nearly fills the pattern doesn't get truncated by that same
  // trailing headroom bar (see channelLoop.ts's channelLoopLength, which
  // this feeds into per-channel).
  //
  // Memoized ($derived): every ChannelRow, the Piano Roll and the audio
  // scheduler (every tick) read this, and as a plain getter each read
  // re-scanned every channel — adding a channel re-ran the full scan once
  // per row. Now it only recomputes on the next read after content changes.
  #activeLength = $derived.by(() => {
    profiler.count('derived.activeLength');
    const bar = this.barLength;
    const patternLength = this.#patternLength;
    let lastOccupiedEnd = 0; // exclusive end step of the last occupied content
    for (const ch of this.channels) {
      if (ch.notes.length > 0) {
        for (const n of ch.notes) {
          if (n.start < patternLength) lastOccupiedEnd = Math.max(lastOccupiedEnd, Math.min(n.start + n.length, patternLength));
        }
      } else {
        for (let i = Math.min(ch.steps.length, patternLength) - 1; i >= 0; i--) {
          if (ch.steps[i]) { lastOccupiedEnd = Math.max(lastOccupiedEnd, i + 1); break; }
        }
      }
    }
    if (lastOccupiedEnd === 0) return bar; // nothing anywhere yet — floor at one bar
    // Round up to the end of the bar containing the last occupied step.
    return Math.min(Math.ceil(lastOccupiedEnd / bar) * bar, patternLength);
  });

  get activeLength(): number {
    return this.#activeLength;
  }

  // True if a channel is currently soloed (Ctrl+click a channel's mute dot
  // in ChannelRow) — a completely separate, Channel-Rack-only mechanism,
  // unrelated to the Mixer's own per-track soloed field. audioEngine.ts
  // reads this indirectly via a getAnySoloed closure passed into
  // start()/startSong() — see playbackStore.svelte.ts — rather than
  // importing channelStore directly, matching the getBpm/getChannels/
  // getBarLength live-getter convention audioEngine.ts already uses
  // throughout instead of reaching into reactive stores itself.
  get anySoloed(): boolean {
    return this.channels.some(c => c.soloed);
  }

  // Single-solo: toggles `id`'s solo on/off, clearing any other channel's
  // solo first — at most one channel can be soloed at a time. Toggling the
  // already-soloed channel again leaves nothing soloed (the "reverse").
  toggleSolo(id: number) {
    const target = this.channels.find(c => c.id === id);
    if (!target) return;
    const wasSoloed = target.soloed;
    for (const c of this.channels) c.soloed = false;
    target.soloed = !wasSoloed;
  }

  // An explicit mute always wins regardless of solo (same rule as
  // mixerStore.isAudible) — a channel that's both muted and soloed stays
  // silent.
  isChannelAudible(ch: ChannelData): boolean {
    return !ch.muted && (!this.anySoloed || ch.soloed);
  }

  // `atIndex` inserts the new channel at that position instead of appending
  // (used when a sample is dropped directly into the Channel Rack list at a
  // specific spot — see ChannelRack.svelte's drop-to-insert handling).
  // Returns the created channel so the caller can set its sample right away.
  addChannel(atIndex?: number): ChannelData {
    const ch = makeChannel(nextId++, this.#stepCount);
    if (atIndex === undefined || atIndex >= this.channels.length) {
      this.channels.push(ch);
    } else {
      this.channels.splice(Math.max(0, atIndex), 0, ch);
    }
    return ch;
  }

  // Removes a channel's settings and every pattern's stashed content for
  // it. The currently-active pattern's content lives directly on the
  // channel object being dropped, so no separate cleanup is needed there.
  removeChannel(id: number) {
    this.channels = this.channels.filter(c => c.id !== id);
    for (const chMap of this.#patternContent.values()) {
      chMap.delete(id);
    }
    if (this.selectedChannelId === id) {
      this.selectedChannelId = null;
    }
  }

  get selectedChannel(): ChannelData | null {
    return this.channels.find(c => c.id === this.selectedChannelId) ?? null;
  }

  // Stashes the current step/note content of every channel under
  // fromPatternId, then restores (or, for a pattern never visited before,
  // creates fresh blank) content for toPatternId. Channels themselves are
  // never added/removed/reordered by this.
  switchPattern(fromPatternId: number, toPatternId: number) {
    const fromContent = new Map<number, PatternContent>();
    for (const ch of this.channels) {
      fromContent.set(ch.id, { steps: ch.steps, notes: ch.notes });
    }
    this.#patternContent.set(fromPatternId, fromContent);

    const toContent = this.#patternContent.get(toPatternId);
    for (const ch of this.channels) {
      const saved = toContent?.get(ch.id);
      ch.steps = saved ? saved.steps : (Array(this.#stepCount).fill(false) as boolean[]);
      ch.notes = saved ? saved.notes : [];
    }
  }

  // Drops a deleted pattern's stashed step/note content (patternStore.
  // removePatterns switches away from an active pattern first, so its live
  // content has already been stashed here by then).
  removePatternContent(patternId: number) {
    this.#patternContent.delete(patternId);
  }

  // Read-only view of a pattern's channels — shared settings merged with
  // that pattern's own step/note content — without disturbing whichever
  // pattern is actually open (unlike switchPattern(), which mutates live
  // state and would visibly disrupt the open Piano Roll/Channel Rack).
  // Used by both audioEngine's song-mode scheduler and the Playlist's
  // combined pattern preview. `selectedPatternId` is taken as an explicit
  // parameter (patternStore.selectedPatternId at the call site) rather than
  // imported here, since patternStore already imports channelStore —
  // importing back would be a circular module dependency for no reason
  // when the caller already has the value in hand. Any reactive caller
  // (a $derived/$effect) that reads selectedPatternId to pass in here
  // naturally reruns on every pattern switch, which is what picks up
  // fresh content the moment switchPattern() stashes it.
  getPatternChannels(patternId: number, selectedPatternId: number): ChannelData[] {
    const isActive = patternId === selectedPatternId;
    return this.channels.map(ch => {
      if (isActive) return ch;
      const saved = this.#patternContent.get(patternId)?.get(ch.id);
      return {
        ...ch,
        steps: saved ? saved.steps : (Array(this.#stepCount).fill(false) as boolean[]),
        notes: saved ? saved.notes : [],
      };
    });
  }

  // Flattens channel settings + every pattern's step/note content (including
  // the currently active pattern, which lives on the live channel objects
  // rather than in #patternContent) into plain, JSON/XML-serializable data.
  exportState(currentPatternId: number): ChannelStoreExport {
    const content: PatternContentEntry[] = [];
    for (const [patternId, chMap] of this.#patternContent) {
      for (const [channelId, c] of chMap) {
        content.push({ patternId, channelId, steps: [...c.steps], notes: c.notes.map(n => ({ ...n })) });
      }
    }
    for (const ch of this.channels) {
      content.push({ patternId: currentPatternId, channelId: ch.id, steps: [...ch.steps], notes: ch.notes.map(n => ({ ...n })) });
    }
    return {
      channels: this.channels.map(c => ({
        id: c.id, samplePath: c.samplePath, sampleFolder: c.sampleFolder, muted: c.muted, soloed: c.soloed,
        pan: c.pan, volume: c.volume, mixerTrack: c.mixerTrack,
        name: c.name, color: c.color,
        midiOutPort: c.midiOutPort, midiOutChannel: c.midiOutChannel,
      })),
      content,
      beatsPerBar: this.#beatsPerBar,
      patternLength: this.#patternLength,
      stepCount: this.#stepCount,
    };
  }

  importState(data: ChannelStoreExport, currentPatternId: number) {
    this.#beatsPerBar = data.beatsPerBar;
    this.#patternLength = data.patternLength;
    // stepCount didn't exist before this feature — #[serde(default)] on the
    // Rust side leaves it 0 for a project saved before then, so fall back to
    // the original fixed size rather than trusting a missing/zero value.
    this.#stepCount = data.stepCount || DEFAULT_STEP_COUNT;

    this.#patternContent = new Map();
    const liveByChannel = new Map<number, PatternContent>();
    for (const raw of data.content) {
      // Notes saved before the note-properties panel existed lack pan/
      // release/mod/color/slide etc. — fill those in at the import boundary.
      const e = { ...raw, notes: (raw.notes ?? []).map(normalizeNote) };
      if (e.patternId === currentPatternId) {
        liveByChannel.set(e.channelId, { steps: e.steps, notes: e.notes });
        continue;
      }
      let chMap = this.#patternContent.get(e.patternId);
      if (!chMap) { chMap = new Map(); this.#patternContent.set(e.patternId, chMap); }
      chMap.set(e.channelId, { steps: e.steps, notes: e.notes });
    }

    this.channels = data.channels.map(c => {
      const live = liveByChannel.get(c.id);
      return {
        // quick-xml round-trips an Option::None field as an empty tag,
        // which deserializes back as "" rather than null/undefined — `||`
        // (not `??`) normalizes that back to null so formatChannelLabel's
        // `sampleFolder ?? samplePath` fallback isn't defeated by a
        // leftover empty string from whichever field wasn't set. Same
        // reasoning applies to name/color, both Option<String> on the Rust
        // side too.
        id: c.id, samplePath: c.samplePath || null, sampleFolder: c.sampleFolder || null, muted: c.muted, soloed: c.soloed,
        pan: c.pan, volume: c.volume, mixerTrack: c.mixerTrack,
        name: c.name || null, color: c.color || null,
        midiOutPort: c.midiOutPort || null, midiOutChannel: c.midiOutChannel || 1,
        steps: live ? live.steps : (Array(this.#stepCount).fill(false) as boolean[]),
        notes: live ? live.notes : [],
      };
    });

    nextId = this.channels.reduce((m, c) => Math.max(m, c.id), 0) + 1;
    // selectedChannelId (which channel the Piano Roll targets) is UI-only —
    // intentionally left untouched.
  }

  resetToDefault() {
    this.#stepCount = DEFAULT_STEP_COUNT;
    this.channels = [makeChannel(0, DEFAULT_STEP_COUNT)];
    this.selectedChannelId = null;
    this.#beatsPerBar = 4;
    this.#patternLength = 16;
    this.#patternContent = new Map();
    nextId = 1;
  }
}

export const channelStore = new ChannelStore();
