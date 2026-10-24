// The live-note pipeline shared by every real-time note source (hardware
// MIDI inputs and the QWERTY "typing keyboard"): holds a voice on the
// selected channel until note-off, and exposes `held` (pitch -> 0..1
// velocity) for the Piano Roll's glowing keys/rows. Notes are tracked per
// (source, pitch) so two devices holding the same key don't cut each other
// off. This is also the hook point future MIDI recording will tap into.
import { SvelteMap } from 'svelte/reactivity';
import { audioEngine } from '$lib/audioEngine';
import { channelStore } from '$lib/channelStore.svelte';

interface HeldVoice {
  source: string;
  pitch: number;
  velocity: number;      // 0..1
  voiceId: number | null;
}

class LiveNotes {
  held = new SvelteMap<number, number>();
  // Only sounds while a project is open (there are no channels on the
  // home screen); the glow/bookkeeping runs regardless.
  active = $state(false);

  #voices = new Map<string, HeldVoice>();
  // Current pitch-bend offset (semitones) per source. Per source, not per
  // voice — a bend wheel moves every note that device is holding, and a
  // note struck while the wheel is already deflected starts bent.
  #bend = new Map<string, number>();

  noteOn(source: string, pitch: number, velocity127: number) {
    const key = `${source}:${pitch}`;
    if (this.#voices.has(key)) this.noteOff(source, pitch); // retrigger
    const velocity = Math.max(0, Math.min(1, velocity127 / 127));
    const ch = channelStore.selectedChannel ?? channelStore.channels[0];
    const bend = this.#bend.get(source) ?? 0;
    const voiceId = this.active && ch ? audioEngine.startLiveVoice(ch, pitch, velocity, bend) : null;
    this.#voices.set(key, { source, pitch, velocity, voiceId });
    this.#refresh(pitch);
  }

  pitchBend(source: string, semitones: number) {
    if (semitones === 0) this.#bend.delete(source);
    else this.#bend.set(source, semitones);
    for (const v of this.#voices.values()) {
      if (v.source === source && v.voiceId !== null) audioEngine.setLiveVoiceBend(v.voiceId, semitones);
    }
  }

  noteOff(source: string, pitch: number) {
    const key = `${source}:${pitch}`;
    const v = this.#voices.get(key);
    if (!v) return;
    if (v.voiceId !== null) audioEngine.releaseLiveVoice(v.voiceId);
    this.#voices.delete(key);
    this.#refresh(pitch);
  }

  // Releases everything held by one source (e.g. a device that was
  // disabled/unplugged mid-note, or the QWERTY keyboard losing focus).
  // Also forgets its bend — a disabled/unplugged device can't send the
  // centering message, so a stale offset would otherwise stick forever.
  releaseSource(source: string) {
    for (const v of [...this.#voices.values()]) {
      if (v.source === source) this.noteOff(source, v.pitch);
    }
    this.#bend.delete(source);
  }

  releaseAll() {
    for (const v of this.#voices.values()) if (v.voiceId !== null) audioEngine.releaseLiveVoice(v.voiceId);
    this.#voices.clear();
    this.#bend.clear();
    this.held.clear();
  }

  #refresh(pitch: number) {
    let max = -1;
    for (const v of this.#voices.values()) if (v.pitch === pitch) max = Math.max(max, v.velocity);
    if (max < 0) this.held.delete(pitch);
    else this.held.set(pitch, max);
  }
}

export const liveNotes = new LiveNotes();
