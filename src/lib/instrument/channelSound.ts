import type { ChannelData } from '$lib/types';

// True when Bloom is assigned to the channel and switched on — the engine
// then plays it through the instrument path.
export function bloomActive(ch: Pick<ChannelData, 'plugin' | 'instrument'>): boolean {
  return ch.plugin === 'bloom' && ch.instrument.enabled;
}

// True when the channel plays an SFZ preset (which then takes priority
// over its sample/folder).
export function usesPreset(ch: Pick<ChannelData, 'plugin' | 'instrument'>): boolean {
  return bloomActive(ch) && ch.instrument.presetPath !== '';
}

// Whether a channel has anything to play at all.
export function channelHasSound(ch: Pick<ChannelData, 'samplePath' | 'sampleFolder' | 'plugin' | 'instrument'>): boolean {
  return !!(ch.samplePath || ch.sampleFolder || usesPreset(ch));
}
