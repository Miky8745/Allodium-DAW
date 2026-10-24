import type { Note } from './types';
import { LIME_NOTE_COLOR, NOTE_COLOR_COUNT } from './noteColorStore.svelte';

// The per-note properties edited by the Piano Roll's properties panel
// (everything on a Note except its id/position/pitch).
export type NoteProps = Pick<Note, 'velocity' | 'pan' | 'release' | 'modX' | 'modY' | 'finePitch' | 'color' | 'slide'>;

export const DEFAULT_NOTE_PROPS: NoteProps = {
  velocity: 0.8,
  pan: 0.5,
  release: 0.5,
  modX: 1,
  modY: 0,
  finePitch: 0,
  color: LIME_NOTE_COLOR,
  slide: false,
};

export const FINE_PITCH_RANGE = 1200; // cents either way

// Fills in properties missing from notes saved before they existed (old
// .alp files / history snapshots), and coerces the numeric ones.
export function normalizeNote(n: Partial<Note> & Pick<Note, 'id' | 'pitch' | 'start' | 'length'>): Note {
  const d = DEFAULT_NOTE_PROPS;
  const num = (v: unknown, fallback: number) => (typeof v === 'number' && Number.isFinite(v) ? v : fallback);
  return {
    id: n.id,
    pitch: n.pitch,
    start: n.start,
    length: n.length,
    velocity: num(n.velocity, d.velocity),
    pan: num(n.pan, d.pan),
    release: num(n.release, d.release),
    modX: num(n.modX, d.modX),
    modY: num(n.modY, d.modY),
    finePitch: num(n.finePitch, d.finePitch),
    color: Math.max(0, Math.min(NOTE_COLOR_COUNT - 1, Math.round(num(n.color, d.color)))),
    slide: n.slide === true,
  };
}

// The note's actual sounding pitch in (fractional) semitones.
export function soundingPitch(n: Pick<Note, 'pitch' | 'finePitch'>): number {
  return n.pitch + n.finePitch / 100;
}

export function formatCents(cents: number): string {
  const c = Math.round(cents);
  return c === 0 ? '0 ct' : `${c > 0 ? '+' : ''}${c} ct`;
}
