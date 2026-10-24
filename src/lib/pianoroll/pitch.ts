import { pianoRollZoom } from '$lib/pianoRollZoom.svelte';

export const MIN_PITCH = 0;
export const MAX_PITCH = 127;
// Base (1x zoom) sizes. Used directly by MiniNoteRoll's Channel Rack preview,
// which must stay a fixed scale regardless of the Piano Roll's own zoom —
// everywhere else should go through keyH()/stepW() below.
export const BASE_KEY_H = 14;  // px per semitone row at 1x
export const BASE_STEP_W = 24; // px per 16th-note step at 1x (matches ChannelRow's .step width)
export const KEY_COL_W = 56;
export const RULER_H = 20;
export const LANE_H = 70;
export const DEFAULT_CENTER_PITCH = 60; // octave convention: 60 = C5

// Live, zoom-scaled sizes — read pianoRollZoom.zoom ($state) so calling these
// inside a template expression/$derived/$effect keeps that context reactive
// to zoom changes, the same way any other $state read would.
export function keyH(): number {
  return BASE_KEY_H * pianoRollZoom.zoom;
}

export function stepW(): number {
  return BASE_STEP_W * pianoRollZoom.zoom;
}

export function gridTotalH(): number {
  return (MAX_PITCH - MIN_PITCH + 1) * keyH();
}

const NOTE_NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];
const BLACK_OFFSETS = new Set([1, 3, 6, 8, 10]);

export function pitchName(pitch: number): string {
  const name = NOTE_NAMES[pitch % 12];
  const octave = Math.floor(pitch / 12);
  return `${name}${octave}`;
}

export function isBlackKey(pitch: number): boolean {
  return BLACK_OFFSETS.has(pitch % 12);
}

export function isCNote(pitch: number): boolean {
  return pitch % 12 === 0;
}

// y=0 is the top of the grid == MAX_PITCH; pitch decreases downward (high notes on top).
export function pitchToY(pitch: number): number {
  return (MAX_PITCH - pitch) * keyH();
}

export function yToPitch(y: number): number {
  const p = MAX_PITCH - Math.floor(y / keyH());
  return Math.max(MIN_PITCH, Math.min(MAX_PITCH, p));
}

// widthPerStep defaults to the live zoomed step width; MiniNoteRoll passes
// BASE_STEP_W explicitly so its Channel Rack preview stays zoom-independent.
export function stepToX(step: number, widthPerStep = stepW()): number {
  return step * widthPerStep;
}

export function xToStep(x: number): number {
  return Math.max(0, Math.floor(x / stepW()));
}

// Unsnapped counterpart to xToStep, used only while Shift is held during a
// drag (free/unsnapped resize or move) — returns a fractional step position
// instead of flooring to a whole step.
export function xToStepFree(x: number): number {
  return Math.max(0, x / stepW());
}

export function patternWidth(patternLength: number, widthPerStep = stepW()): number {
  return patternLength * widthPerStep;
}

export function clampPitch(pitch: number): number {
  return Math.max(MIN_PITCH, Math.min(MAX_PITCH, pitch));
}

const NOTE_BASE: Record<string, number> = { c: 0, d: 2, e: 4, f: 5, g: 7, a: 9, b: 11 };

// Matches a trailing note-name token like "C1", "G#3", "A_2" (some sample
// libraries use "_" instead of "#" to keep sharps filesystem-safe). Requires the digit
// group to sit directly against the letter/accidental — this is what keeps
// it from firing on ordinary words (e.g. "Piano", "Guitar").
const NOTE_TOKEN = /([A-Ga-g])(#|_)?(-?\d{1,2})(?![A-Za-z0-9])/g;

// Recovers the recorded pitch from a multisample filename, e.g.
// "01_SAX_LONG G#1ogg.wav" -> pitch for G#1. Some libraries concatenate the
// original ".ogg" extension's letters onto the note name when re-saved as
// ".wav" (no separating dot), so that noise is stripped first. Returns null
// when no note token is found — such files are round-robin/one-shot samples
// with no pitch of their own (see multisample.ts).
export function parseNoteName(filename: string): number | null {
  const stem = filename.replace(/\.[^.]+$/, '').replace(/ogg$/i, '');
  let match: RegExpExecArray | null;
  let last: RegExpExecArray | null = null;
  NOTE_TOKEN.lastIndex = 0;
  while ((match = NOTE_TOKEN.exec(stem)) !== null) last = match;
  if (!last) return null;
  const base = NOTE_BASE[last[1].toLowerCase()];
  const accidental = last[2] ? 1 : 0;
  const octave = parseInt(last[3], 10);
  return clampPitch(octave * 12 + base + accidental);
}
