import type { ChannelData } from './types';
import { usesPreset } from './instrument/channelSound';
import { presetNameFromPath } from './instrument/sfz';
import { channelPlugin } from './plugins/registry';

export function formatSampleName(path: string | null): string {
  if (!path) return 'Drop sample';
  const base = path.split('/').pop() ?? path;
  const noExt = base.replace(/\.[^.]+$/, '');
  return noExt
    .replace(/[-_]/g, ' ')
    .replace(/\b\w/g, c => c.toUpperCase());
}

// A channel is either a single sample or a multisample folder; this picks
// whichever is set for display (ChannelRow's drop zone, the Piano Roll's
// header) rather than every caller re-checking both fields. A user-set
// `name` (right-click > Rename in ChannelRow) always wins over the
// sample-derived label.
// A channel with a plugin assigned is labeled with the plugin's name.
export function formatChannelLabel(channel: Pick<ChannelData, 'samplePath' | 'sampleFolder' | 'name' | 'plugin' | 'instrument'>): string {
  if (channel.name) return channel.name;
  const plugin = channelPlugin(channel);
  if (plugin) return plugin.name;
  return formatSampleName(channel.sampleFolder ?? channel.samplePath);
}

// What a channel actually plays — the preset name when Bloom plays one,
// otherwise the sample/folder (the Bloom window's header/source line).
export function formatSourceLabel(channel: Pick<ChannelData, 'samplePath' | 'sampleFolder' | 'plugin' | 'instrument'>): string {
  if (usesPreset(channel)) return presetNameFromPath(channel.instrument.presetPath);
  return formatSampleName(channel.sampleFolder ?? channel.samplePath);
}
