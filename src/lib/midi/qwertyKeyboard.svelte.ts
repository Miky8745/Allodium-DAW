// The Toolbar's "Typing keyboard to piano" toggle: turns the computer
// keyboard into a virtual MIDI input, feeding the same liveNotes pipeline
// hardware MIDI does. Tracker-style layout — two rows per octave, by physical
// key position (KeyboardEvent.code, so it works on any keyboard layout):
//   lower octave: Z S X D C V G B H N J M   (white row Z..M, black S D G H J)
//   upper octave: Q 2 W 3 E R 5 T 6 Y 7 U I (white row Q..I, black 2 3 5 6 7)
// `,` / `.` shift the octave down/up.
import { liveNotes } from './liveNotes.svelte';
import { preferences } from '$lib/preferencesStore.svelte';

export const QWERTY_SOURCE = 'Typing keyboard';

const KEY_OFFSETS: Record<string, number> = {
  KeyZ: 0, KeyS: 1, KeyX: 2, KeyD: 3, KeyC: 4, KeyV: 5, KeyG: 6, KeyB: 7, KeyH: 8, KeyN: 9, KeyJ: 10, KeyM: 11,
  KeyQ: 12, Digit2: 13, KeyW: 14, Digit3: 15, KeyE: 16, KeyR: 17, Digit5: 18, KeyT: 19, Digit6: 20, KeyY: 21, Digit7: 22, KeyU: 23, KeyI: 24,
};

const BASE_PITCH = 60; // C5, the app's unshifted reference (see pianoroll/pitch.ts)

class QwertyKeyboard {
  enabled = $state(false);
  octave = $state(0); // shift from BASE_PITCH, in octaves

  // code -> pitch it started, so a key released after an octave shift
  // still releases the note it actually played.
  #down = new Map<string, number>();

  toggle() {
    this.enabled = !this.enabled;
    if (!this.enabled) this.releaseAll();
  }

  // Returns true if the event was consumed.
  keyDown(e: KeyboardEvent): boolean {
    if (!this.enabled) return false;
    if (e.code === 'Comma' || e.code === 'Period') {
      if (!e.repeat) this.octave = Math.max(-5, Math.min(4, this.octave + (e.code === 'Comma' ? -1 : 1)));
      return true;
    }
    const offset = KEY_OFFSETS[e.code];
    if (offset === undefined) return false;
    if (e.repeat || this.#down.has(e.code)) return true;
    const pitch = BASE_PITCH + this.octave * 12 + offset;
    if (pitch < 0 || pitch > 127) return true;
    this.#down.set(e.code, pitch);
    liveNotes.noteOn(QWERTY_SOURCE, pitch, preferences.general.qwertyVelocity);
    return true;
  }

  keyUp(e: KeyboardEvent): boolean {
    const pitch = this.#down.get(e.code);
    if (pitch === undefined) return false;
    this.#down.delete(e.code);
    liveNotes.noteOff(QWERTY_SOURCE, pitch);
    return true;
  }

  releaseAll() {
    this.#down.clear();
    liveNotes.releaseSource(QWERTY_SOURCE);
  }
}

export const qwertyKeyboard = new QwertyKeyboard();
