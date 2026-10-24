import { untrack } from 'svelte';
import { audioEngine } from './audioEngine';
import { channelStore } from './channelStore.svelte';
import { patternStore } from './patternStore.svelte';
import { getSongPlacements, getSongLengthSteps } from './songTimeline.svelte';
import { profiler } from './debug/profiler';

// The single, unified playback clock. Owns the live rAF loop + audioEngine
// start/stop orchestration (moved out of ChannelRack.svelte, which used to
// run this itself — meaning playback silently stopped advancing its visual
// position if that window was ever closed). Mirrors historyStore.svelte.ts's
// $effect.root() pattern for a module-singleton's live reactive effect.
class PlaybackStore {
  isPlaying = $state(false);
  tempo = $state(120);
  currentStep = $state(-1);          // floored step, existing consumers (discrete highlights)
  currentStepFraction = $state(0);   // continuous position, for the smooth playhead line
  soloChannelId = $state<number | null>(null); // set by the Piano Roll's own play button
  transportMode = $state<'pattern' | 'song'>('pattern');
  metronomeOn = $state(false); // Toolbar's metronome toggle
  seekStep = $state(0); // Playlist start marker (steps) — where Song-mode playback begins

  #rafId = 0;
  #stopEffect: (() => void) | null = null;

  #currentChannels() {
    return this.soloChannelId != null
      ? channelStore.channels.filter(c => c.id === this.soloChannelId)
      : channelStore.channels;
  }

  startWatching() {
    if (this.#stopEffect) return;
    this.#stopEffect = $effect.root(() => {
      $effect(() => {
        // Read as a dependency (not just for its value) so flipping the
        // Toolbar's PAT/SONG toggle *while playing* restarts the transport
        // in the new mode immediately, instead of only taking effect on the
        // next manual play/stop.
        const mode = this.transportMode;
        if (this.isPlaying) {
          audioEngine.stop();
          if (mode === 'song') {
            audioEngine.startSong(
              () => this.tempo,
              () => getSongPlacements(),
              (patternId) => channelStore.getPatternChannels(patternId, patternStore.selectedPatternId),
              () => getSongLengthSteps(),
              () => channelStore.barLength,
              () => this.metronomeOn,
              () => { this.isPlaying = false; }, // stop-at-end: same end state as pressing Stop
              // Untracked: the marker moves while playing too, and that must not
              // restart the transport (the click handler jumps the playhead itself).
              untrack(() => this.seekStep),
              () => channelStore.anySoloed,
            );
          } else {
            audioEngine.start(
              () => this.tempo,
              () => this.#currentChannels(),
              () => channelStore.activeLength,
              () => channelStore.barLength,
              () => this.metronomeOn,
              () => channelStore.anySoloed,
            );
          }
          this.#startRaf();
        } else {
          audioEngine.stop();
          this.#stopRaf();
          this.currentStep = -1;
          this.currentStepFraction = 0;
        }
      });
    });
  }

  stopWatching() {
    this.#stopEffect?.();
    this.#stopEffect = null;
    this.#stopRaf();
    audioEngine.stop();
  }

  #startRaf() {
    const frame = () => {
      if (!this.isPlaying) return;
      const t = profiler.begin();
      const { step, fraction } = audioEngine.getPosition();
      if (step >= 0) {
        this.currentStep = step;
        this.currentStepFraction = fraction;
      }
      profiler.end('clock.raf', t);
      this.#rafId = requestAnimationFrame(frame);
    };
    cancelAnimationFrame(this.#rafId);
    this.#rafId = requestAnimationFrame(frame);
  }

  #stopRaf() {
    cancelAnimationFrame(this.#rafId);
    this.#rafId = 0;
  }

  // Play/pause toggle for keyboard shortcuts — same effect as the Toolbar's
  // Play button (full-pattern playback, so any Piano Roll solo is cleared).
  togglePlay() {
    this.soloChannelId = null;
    this.isPlaying = !this.isPlaying;
  }

  // Moves the running playhead without touching the start marker (`seekStep`).
  // Song mode only — pattern mode has no timeline position to jump within.
  jumpTo(step: number) {
    if (!this.isPlaying || this.transportMode !== 'song') return;
    audioEngine.seekToStep(step);
  }

  // Called whenever the active pattern changes (patternStore's
  // selectedPatternId setter) so the clock never keeps counting through a
  // pattern switch — if playing, actually restarts the audible loop at step
  // 0 of the new pattern; if stopped, just snaps the visual marker to idle.
  restartFromZero() {
    if (this.isPlaying) {
      audioEngine.stop();
      audioEngine.start(
        () => this.tempo,
        () => this.#currentChannels(),
        () => channelStore.activeLength,
        () => channelStore.barLength,
        () => this.metronomeOn,
        () => channelStore.anySoloed,
      );
    } else {
      this.currentStep = -1;
      this.currentStepFraction = 0;
    }
  }
}

export const playback = new PlaybackStore();
