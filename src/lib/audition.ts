import type { ChannelData } from './types';
import { audioEngine } from './audioEngine';

// Click-to-hear in the Channel Rack: pressing a step or a channel's sample
// field plays that channel's step sound (pitch 60, full velocity, through
// its mixer track — or its plugin) and releasing the mouse anywhere stops
// it. One audition at a time; a new press replaces the old one.
let voiceId: number | null = null;

function stop() {
  window.removeEventListener('pointerup', stop);
  window.removeEventListener('pointercancel', stop);
  window.removeEventListener('blur', stop);
  if (voiceId !== null) audioEngine.releaseLiveVoice(voiceId, true);
  voiceId = null;
}

export function auditionChannel(ch: ChannelData): void {
  stop();
  voiceId = audioEngine.startLiveVoice(ch, 60, 1, 0, true);
  window.addEventListener('pointerup', stop);
  window.addEventListener('pointercancel', stop);
  window.addEventListener('blur', stop);
}
