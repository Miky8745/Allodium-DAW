import type { ChannelData, FileDropDetail } from './types';
import { audioEngine } from './audioEngine';
import { applyPreset } from './instrument/applyPreset';

export const isPresetPath = (path: string) => /\.sfz$/i.test(path);

// What dropping a file/folder from the explorer onto a channel does: an
// .sfz loads as an instrument preset; an audio file/folder becomes the
// channel's sample/multisample (and drops any preset, which would
// otherwise keep taking priority).
export function applyDropToChannel(ch: ChannelData, drop: FileDropDetail): void {
  if (drop.kind === 'file' && isPresetPath(drop.path)) {
    void applyPreset(ch.id, drop.path);
    return;
  }
  if (drop.kind === 'file') {
    ch.samplePath = drop.path;
    ch.sampleFolder = null;
  } else {
    ch.sampleFolder = drop.path;
    ch.samplePath = null;
  }
  ch.instrument.presetPath = '';
  audioEngine.preloadChannel(ch);
}
