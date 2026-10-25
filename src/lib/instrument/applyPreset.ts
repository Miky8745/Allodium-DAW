import { audioEngine } from '$lib/audioEngine';
import { channelStore } from '$lib/channelStore.svelte';
import { loadPreset } from './presetLoader';
import { presetDefaults } from './voiceShape';
import { defaultInstrument } from './types';

// Loads an .sfz preset onto a channel: assigns Bloom and resets
// its envelope/filter/macros to the preset's own values. Looks the channel
// up by id after the async load so the write goes through the store's
// reactive proxy. Returns false if the preset couldn't be read.
export async function applyPreset(channelId: number, path: string): Promise<boolean> {
  const preset = await loadPreset(path);
  const ch = channelStore.channels.find(c => c.id === channelId);
  if (!preset || !ch) return false;
  ch.plugin = 'bloom';
  ch.instrument = { ...defaultInstrument(), ...presetDefaults(preset), enabled: true, presetPath: path };
  audioEngine.preloadChannel(ch);
  return true;
}

// Bloom's Init button: settings back to defaults (preset removed), still on.
export function resetInstrument(channelId: number): void {
  const ch = channelStore.channels.find(c => c.id === channelId);
  if (ch) ch.instrument = { ...defaultInstrument(), enabled: true };
}
