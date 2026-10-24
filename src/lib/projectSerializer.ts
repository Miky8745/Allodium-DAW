import { invoke } from '@tauri-apps/api/core';
import { channelStore, type ChannelSettingsExport, type PatternContentEntry } from './channelStore.svelte';
import { patternStore } from './patternStore.svelte';
import { placementStore } from './placementStore.svelte';
import { playback } from './playbackStore.svelte';
import { noteColorStore } from './noteColorStore.svelte';
import type { PatternData, Placement } from './types';

// Mirrors src-tauri/src/project.rs's `ProjectFile` field-for-field (including
// the wrapper objects quick-xml needs to give each list a clean per-item XML
// tag) so the plain object passed to `invoke('save_project', ...)` matches
// what serde expects on the Rust side.
export interface ProjectFileData {
  beatsPerBar: number;
  patternLength: number;
  stepCount: number;
  tempo: number;
  selectedPatternId: number;
  channels: { channel: ChannelSettingsExport[] };
  content: { entry: PatternContentEntry[] };
  patterns: { pattern: PatternData[] };
  placements: { placement: Placement[] };
  // Piano Roll note palette (15 hex colors). Optional: absent in history
  // snapshots taken before it existed.
  noteColors?: { color: string[] };
}

export function serializeProject(): ProjectFileData {
  const channelsExport = channelStore.exportState(patternStore.selectedPatternId);
  const patternsExport = patternStore.exportState();
  return {
    beatsPerBar: channelsExport.beatsPerBar,
    patternLength: channelsExport.patternLength,
    stepCount: channelsExport.stepCount,
    tempo: playback.tempo,
    selectedPatternId: patternsExport.selectedPatternId,
    channels: { channel: channelsExport.channels },
    content: { entry: channelsExport.content },
    patterns: { pattern: patternsExport.patterns },
    placements: { placement: placementStore.exportState() },
    noteColors: { color: noteColorStore.exportState() },
  };
}

export function applyProjectState(data: ProjectFileData) {
  patternStore.importState({ patterns: data.patterns.pattern, selectedPatternId: data.selectedPatternId });
  channelStore.importState(
    {
      channels: data.channels.channel,
      content: data.content.entry,
      beatsPerBar: data.beatsPerBar,
      patternLength: data.patternLength,
      stepCount: data.stepCount,
    },
    data.selectedPatternId,
  );
  placementStore.importState(data.placements.placement);
  noteColorStore.importState(data.noteColors?.color);
  playback.tempo = data.tempo;
}

// Resets all four stores to a blank project — needed because they're
// process-wide singletons that would otherwise leak a previously open
// project's state into a newly opened one that has no saved .alp yet.
export function resetAllStores() {
  channelStore.resetToDefault();
  patternStore.resetToDefault();
  placementStore.resetToDefault();
  noteColorStore.resetToDefault();
  playback.tempo = 120;
}

export async function saveProject(name: string): Promise<void> {
  await invoke('save_project', { name, project: serializeProject() });
}

// Returns true if a saved .alp was found and applied; false if the project
// has never been saved (caller should call resetAllStores() in that case).
export async function loadProject(name: string): Promise<boolean> {
  const loaded = await invoke<ProjectFileData | null>('load_project', { name });
  if (loaded) {
    applyProjectState(loaded);
    return true;
  }
  return false;
}
