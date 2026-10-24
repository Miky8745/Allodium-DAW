// Frontend half of the MIDI system: bridges Rust's midi.rs events into the
// stores, keeps the backend's open connections in sync with which devices
// are enabled in Preferences (re-applied on every hot-plug change, which is
// what makes an unplugged-then-replugged device reconnect on its own), and
// routes processed note on/off into liveNotes. Started once at app mount —
// not per project — so the Preferences MIDI tab works from the home screen.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { audioEngine } from '$lib/audioEngine';
import { preferences } from '$lib/preferencesStore.svelte';
import { midiStore, type MidiPorts } from './midiStore.svelte';
import { liveNotes } from './liveNotes.svelte';
import { describeMidi, parseMidi } from './midiParse';
import { processInputNote, processPitchBend } from './midiProcessing';
import { sendNote, stopAll } from './midiOutput';

interface MidiMessageEvent {
  port: string;
  bytes: number[];
}

class MidiSystem {
  #unlisten: UnlistenFn[] = [];
  #stopEffect: (() => void) | null = null;
  // `${port}:${channel}:${note}` -> the processed pitch its note-on played,
  // so the matching note-off releases the same pitch even if the device's
  // transpose was changed while the key was held.
  #sounding = new Map<string, number>();
  #prevInputs = new Set<string>();

  async start() {
    if (this.#stopEffect) return;
    audioEngine.setNoteSink(sendNote);
    audioEngine.setStopHook(stopAll);

    this.#unlisten.push(
      await listen<MidiMessageEvent>('midi-message', e => this.#onMessage(e.payload)),
      await listen<MidiPorts>('midi-ports-changed', e => {
        midiStore.ports = e.payload;
        void this.#apply();
      }),
    );

    this.#stopEffect = $effect.root(() => {
      $effect(() => {
        // Dependencies: the enabled-name lists only (not every setting).
        const inputs = preferences.enabledInputNames;
        const outputs = preferences.enabledOutputNames;
        void this.#apply(inputs, outputs);
      });
    });
  }

  stop() {
    this.#unlisten.forEach(u => u());
    this.#unlisten = [];
    this.#stopEffect?.();
    this.#stopEffect = null;
    audioEngine.setNoteSink(null);
    audioEngine.setStopHook(null);
  }

  async rescan() {
    midiStore.ports = await invoke<MidiPorts>('midi_list_ports');
    await this.#apply();
  }

  async #apply(inputs = preferences.enabledInputNames, outputs = preferences.enabledOutputNames) {
    const wanted = new Set(inputs);
    for (const name of this.#prevInputs) {
      if (!wanted.has(name)) this.#releasePort(name);
    }
    this.#prevInputs = wanted;
    try {
      await invoke('midi_set_inputs', { names: inputs });
      midiStore.ports = await invoke<MidiPorts>('midi_set_outputs', { names: outputs });
    } catch (e) {
      console.error('Failed to apply MIDI ports', e);
    }
  }

  #releasePort(port: string) {
    liveNotes.releaseSource(port);
    for (const key of [...this.#sounding.keys()]) if (key.startsWith(`${port}:`)) this.#sounding.delete(key);
  }

  #onMessage({ port, bytes }: MidiMessageEvent) {
    const msg = parseMidi(bytes);
    if (!msg) return;
    midiStore.record('in', port, describeMidi(msg));
    if (msg.type !== 'noteOn' && msg.type !== 'noteOff' && msg.type !== 'pitchBend') return; // CC/etc: activity + log only, for now
    const prefs = preferences.findInput(port);
    if (!prefs?.enabled) return;

    if (msg.type === 'pitchBend') {
      const semis = processPitchBend(msg.channel, msg.data1, msg.data2, prefs);
      if (semis !== null) liveNotes.pitchBend(port, semis);
      return;
    }

    const key = `${port}:${msg.channel}:${msg.data1}`;
    if (msg.type === 'noteOn') {
      const processed = processInputNote(msg.channel, msg.data1, msg.data2, true, prefs);
      if (!processed) return;
      this.#sounding.set(key, processed.pitch);
      liveNotes.noteOn(port, processed.pitch, processed.velocity);
    } else {
      const pitch = this.#sounding.get(key);
      if (pitch === undefined) return;
      this.#sounding.delete(key);
      liveNotes.noteOff(port, pitch);
    }
  }
}

export const midiSystem = new MidiSystem();
