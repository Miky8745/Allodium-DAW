// Ephemeral MIDI runtime state (session-only, never saved): what ports the
// backend currently sees, which are connected, per-port activity for the
// LEDs, and the Preferences > MIDI monitor log. Device *settings* live in
// preferencesStore instead — they're remembered across sessions by name.
import { SvelteMap } from 'svelte/reactivity';

export interface MidiPorts {
  inputs: string[];
  outputs: string[];
  connectedInputs: string[];
  connectedOutputs: string[];
}

export interface MidiLogEntry {
  id: number;
  time: number;          // performance.now()
  direction: 'in' | 'out';
  port: string;
  text: string;
}

const LOG_SIZE = 100;
let nextLogId = 1;

class MidiStore {
  ports = $state<MidiPorts>({ inputs: [], outputs: [], connectedInputs: [], connectedOutputs: [] });
  log = $state<MidiLogEntry[]>([]);
  // port key -> performance.now() of its last message, in either direction
  activity = new SvelteMap<string, number>();
  lastActivity = $state(0);
  lastMessage = $state<string | null>(null);

  record(direction: 'in' | 'out', port: string, text: string) {
    const time = performance.now();
    this.log.push({ id: nextLogId++, time, direction, port, text });
    if (this.log.length > LOG_SIZE) this.log.splice(0, this.log.length - LOG_SIZE);
    this.activity.set(port, time);
    this.lastActivity = time;
    if (direction === 'in') this.lastMessage = `${text} (${port})`;
  }

  clearLog() {
    this.log = [];
  }

  isInputConnected(name: string) {
    return this.ports.connectedInputs.includes(name);
  }

  isOutputConnected(name: string) {
    return this.ports.connectedOutputs.includes(name);
  }
}

export const midiStore = new MidiStore();
