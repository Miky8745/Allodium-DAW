import type { SfzInstrument } from './sfz';
import type { VoiceShape } from './voiceShape';
import type { InstrumentHit } from './voiceGraph';
import { selectRegions, velocityGain } from './regionSelect';

// The samples one note of a preset plays (one per matching region — SFZ
// layers every match). `pitch` may be fractional (Piano Roll fine pitch).
// `player` (channel id) scopes keyswitch state.
export function presetHits(preset: SfzInstrument, pitch: number, velocity: number, shape: VoiceShape, player: number): InstrumentHit[] {
  return selectRegions(preset, pitch, velocity, player).map(r => ({
    path: r.sample,
    rate: 2 ** (((pitch - r.keycenter) * r.pitchKeytrack + r.tune + shape.pitchCents) / 1200),
    gain: 10 ** (r.volume / 20) * velocityGain(r, velocity),
    pan: r.pan / 100,
    loop: r.loop,
    offset: r.offset,
    oneShot: r.oneShot,
  }));
}

// An instrument channel without a preset: the channel's own sample/folder
// pick (`rate` already pitched), shaped by the instrument.
export function sampleHit(path: string, rate: number, shape: VoiceShape): InstrumentHit {
  return { path, rate: rate * 2 ** (shape.pitchCents / 1200), gain: 1, pan: 0, loop: null, offset: 0, oneShot: false };
}
