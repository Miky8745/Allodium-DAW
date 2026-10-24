// Pure per-device note processing — no stores, no side effects — so the
// Preferences MIDI tab's settings map 1:1 onto what these functions do.
import type { MidiInputPrefs, MidiOutputPrefs, VelocityCurve } from '$lib/preferencesTypes';

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

// Exponent applied to normalized velocity (0..1): <1 boosts soft playing
// ("soft" curve — light touch reads louder), >1 needs harder hits.
export function curveExponent(curve: VelocityCurve, custom: number): number {
  switch (curve) {
    case 'soft': return 0.6;
    case 'hard': return 1.6;
    case 'custom': return clamp(custom, 0.1, 5);
    default: return 1;
  }
}

export function shapeVelocity(raw: number, p: MidiInputPrefs): number {
  if (p.fixedVelocity > 0) return clamp(p.fixedVelocity, 1, 127);
  const curved = Math.pow(clamp(raw, 0, 127) / 127, curveExponent(p.velCurve, p.velExponent)) * 127;
  const lo = clamp(Math.min(p.velMin, p.velMax), 1, 127);
  const hi = clamp(Math.max(p.velMin, p.velMax), 1, 127);
  return Math.round(clamp(curved + p.velOffset, lo, hi));
}

export interface ProcessedNote {
  pitch: number;     // after transpose, 0-127
  velocity: number;  // 1-127 (note-on only; 0 for note-off)
}

// Returns null when the note should be dropped (wrong channel, outside the
// key range, or transposed off the end of the MIDI range). The range is
// checked on the *incoming* key, before transpose — it's a split point on
// the physical keyboard.
export function processInputNote(channel: number, note: number, velocity: number, isOn: boolean, p: MidiInputPrefs): ProcessedNote | null {
  if (p.channel !== 0 && channel !== p.channel) return null;
  const lo = Math.min(p.noteLow, p.noteHigh);
  const hi = Math.max(p.noteLow, p.noteHigh);
  if (note < lo || note > hi) return null;
  const pitch = note + p.transpose;
  if (pitch < 0 || pitch > 127) return null;
  return { pitch, velocity: isOn ? shapeVelocity(velocity, p) : 0 };
}

// 14-bit pitch bend (lsb, msb) → semitones, scaled so full deflection in
// either direction is exactly ±bendRange. Null on a filtered-out channel.
export function processPitchBend(channel: number, lsb: number, msb: number, p: MidiInputPrefs): number | null {
  if (p.channel !== 0 && channel !== p.channel) return null;
  const raw = ((msb << 7) | lsb) - 8192;
  return (raw / (raw < 0 ? 8192 : 8191)) * clamp(p.bendRange, 0, 48);
}

export function processOutputNote(pitch: number, velocity127: number, p: MidiOutputPrefs | undefined): ProcessedNote | null {
  const out = pitch + (p?.transpose ?? 0);
  if (out < 0 || out > 127) return null;
  return { pitch: out, velocity: clamp(Math.round(velocity127 * (p?.velScale ?? 1)), 1, 127) };
}
