// Pushes Preferences > General values into the systems they configure.
// One $effect per concern (same convention as mixerAudio.svelte.ts) so e.g.
// moving the GUI-scale preset doesn't rebuild the AudioContext. Started once
// at app mount, after preferences.load().
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { audioEngine } from './audioEngine';
import { playback } from './playbackStore.svelte';
import { preferences } from './preferencesStore.svelte';
import { untrack } from 'svelte';

let stop: (() => void) | null = null;

export function startApplyingPreferences() {
  if (stop) return;
  stop = $effect.root(() => {
    $effect(() => {
      const scale = preferences.general.guiScale;
      getCurrentWebview().setZoom(scale).catch(e => console.error('setZoom failed', e));
    });

    $effect(() => {
      const latencyHint = preferences.general.latencyHint;
      const sampleRate = preferences.general.sampleRate;
      // A rebuild kills the running transport's graph, so stop playback
      // first (playbackStore owns isPlaying; its watcher calls
      // audioEngine.stop()).
      untrack(() => { if (playback.isPlaying) playback.isPlaying = false; });
      void audioEngine.rebuildContext({ latencyHint, sampleRate });
    });

    $effect(() => {
      audioEngine.setSchedulerTiming(preferences.general.lookaheadMs, preferences.general.tickMs);
    });

    $effect(() => {
      audioEngine.setPreview(preferences.general.previewVolume, preferences.general.previewLengthMs);
    });
  });
}
