// Mirrors piano-roll notes out as MIDI (never step-sequencer hits — see
// audioEngine.scheduleChannelStep's noteSink call). Registered as
// audioEngine's NoteSink/stop hook by midiSystem.start(). Scheduled
// playback notes arrive ~lookahead ahead of time, so each note-on/off pair
// is converted to a wall-clock delay and handed to Rust's scheduler thread
// (midi.rs's midi_schedule) in one batched IPC per scheduler tick.
import { invoke } from '@tauri-apps/api/core';
import { audioEngine } from '$lib/audioEngine';
import { preferences } from '$lib/preferencesStore.svelte';
import type { ChannelData } from '$lib/types';
import { midiStore } from './midiStore.svelte';
import { noteOffBytes, noteOnBytes } from './midiParse';
import { processOutputNote } from './midiProcessing';
import { pitchName } from '$lib/pianoroll/pitch';

interface ScheduledEvent {
  port: string;
  bytes: number[];
  delayMs: number;
}

let batch: ScheduledEvent[] = [];
let flushQueued = false;

function flush() {
  flushQueued = false;
  if (batch.length === 0) return;
  const events = batch;
  batch = [];
  invoke('midi_schedule', { events }).catch(e => console.error('midi_schedule failed', e));
}

function enabledOutput(port: string | null) {
  if (!port) return undefined;
  const p = preferences.findOutput(port);
  return p?.enabled ? p : undefined;
}

export function sendNote(ch: ChannelData, pitch: number, velocity01: number, playTime: number, durationSeconds: number) {
  const out = enabledOutput(ch.midiOutPort);
  if (!out) return;
  const processed = processOutputNote(pitch, velocity01 * 127, out);
  if (!processed) return;
  // AudioContext time -> ms from now, plus the context's own output latency
  // (so MIDI lines up with what's actually heard) and the per-port offset.
  const delay = (playTime - audioEngine.currentTime + audioEngine.outputLatency) * 1000 + out.latencyMs;
  const port = out.name;
  batch.push(
    { port, bytes: noteOnBytes(ch.midiOutChannel, processed.pitch, processed.velocity), delayMs: delay },
    { port, bytes: noteOffBytes(ch.midiOutChannel, processed.pitch), delayMs: delay + durationSeconds * 1000 },
  );
  midiStore.record('out', port, `Note ${pitchName(processed.pitch)} vel ${processed.velocity} · ch ${ch.midiOutChannel}`);
  if (!flushQueued) {
    flushQueued = true;
    queueMicrotask(flush);
  }
}

// Transport stop: cancel everything still queued and silence what already
// went out (Rust tracks sounding notes per port), plus CC123 on ports that
// ask for it.
export function stopAll() {
  batch = [];
  const cc123Ports = preferences.outputs.filter(o => o.enabled && o.allNotesOffOnStop).map(o => o.name);
  invoke('midi_stop_all', { cc123Ports }).catch(e => console.error('midi_stop_all failed', e));
}
