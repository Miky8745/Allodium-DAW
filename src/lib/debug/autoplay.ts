import { invoke } from '@tauri-apps/api/core';
import { profiler } from './profiler';
import { playback } from '$lib/playbackStore.svelte';
import { audioEngine } from '$lib/audioEngine';
import { mixerStore } from '$lib/mixerStore.svelte';

// Dev-only harness (see src-tauri/src/debug.rs): starts the profiler and,
// when ALLODIUM_AUTOPLAY names a project, plays it — Song mode for
// `autoplaySeconds` (restarting whenever the song ends), then pattern mode
// for half as long as a baseline — muted, and quits the app. Run it on a
// virtual display (xvfb-run, see CLAUDE.md) — a minimized/hidden WebKitGTK
// window throttles timers to ~1s and stops rAF, which wrecks the numbers.
interface DebugConfig {
  profile: boolean;
  autoplay: string;
  autoplaySeconds: number;
  panels: string;
}

const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));

export async function runDebugHarness(openProject: (name: string) => Promise<void>, currentProject: () => string, showPanels: (names: string[]) => void) {
  const cfg = await invoke<DebugConfig>('debug_config');
  if (!cfg.profile && !cfg.autoplay) return;
  profiler.start();
  profiler.gauge('audio', () => `${audioEngine.contextState} lead=${audioEngine.schedulerLeadMs.toFixed(0)}ms voices=${audioEngine.activeVoices}`);
  profiler.gauge('dom', () => document.getElementsByTagName('*').length);
  profiler.gauge('step', () => playback.currentStep);
  if (!cfg.autoplay) return;

  if (currentProject() !== cfg.autoplay) await openProject(cfg.autoplay);
  // Silent: the full audio graph still runs, it just doesn't reach the
  // speakers (the Mixer's master isn't saved, so this doesn't stick).
  mixerStore.master.volume = 0;
  if (cfg.panels) showPanels(cfg.panels === 'all' ? ['channelrack', 'pianoroll', 'mixer', 'overview'] : cfg.panels.split(','));
  await sleep(2000); // let samples load and the UI settle

  await phase('song', cfg.autoplaySeconds);
  await phase('pattern', cfg.autoplaySeconds / 2);

  profiler.event('autoplay done');
  await profiler.drain();
  await invoke('debug_exit');
}

async function phase(mode: 'song' | 'pattern', seconds: number) {
  playback.transportMode = mode;
  playback.seekStep = 0;
  profiler.event(`phase ${mode} start (${seconds}s)`);
  playback.isPlaying = true;
  const end = performance.now() + seconds * 1000;
  while (performance.now() < end) {
    await sleep(100);
    if (!playback.isPlaying) {
      profiler.event('song ended — restarting');
      playback.isPlaying = true;
    }
  }
  playback.isPlaying = false;
  profiler.event(`phase ${mode} end`);
  await sleep(500);
}
