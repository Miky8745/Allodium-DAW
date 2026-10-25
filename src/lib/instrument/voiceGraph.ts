import type { VoiceShape } from './voiceShape';
import { scheduleAdsr, scheduleFilterEnv } from './envelope';

// One sample to play for an instrument note — a matched SFZ region, or the
// channel's own sample/folder when no preset is loaded.
export interface InstrumentHit {
  path: string;
  rate: number;    // playback rate (pitch, region tune and channel tune included)
  gain: number;    // linear multiplier (region volume)
  pan: number;     // -1..1 offset (region pan)
  loop: { start: number; end: number } | 'whole' | null; // frames at source rate
  offset: number;  // frames at source rate
  oneShot: boolean;
}

export interface VoiceParams {
  volume: number;        // linear peak level (channel volume × velocity, before hit/shape gain)
  pan: number;           // 0..1 channel(+note) pan
  time: number;
  duration: number | null; // seconds; null = held live voice
  shape: VoiceShape;
  sourceRate: number;    // for loop points/offset
  // Per-note filter modifiers (Piano Roll mod X/Y): multiply the cutoff,
  // add resonance (dB). 1/0 = unchanged.
  cutoffScale: number;
  resonanceAdd: number;
  releaseScale: number;  // Piano Roll note "rel" property, 1 = unchanged
}

export interface BuiltVoice {
  src: AudioBufferSourceNode;
  gain: GainNode;
  filter: BiquadFilterNode | null;
  panner: StereoPannerNode;
  end: number;      // when the voice is silent (Infinity while held)
  release: number;  // seconds, for releasing a held voice
}

const dbToGain = (db: number) => 10 ** (db / 20);

// Builds and schedules one instrument voice: source → gain (amp envelope)
// → filter (filter envelope) → panner → dest. The caller owns onended
// cleanup, slide registration and voice counting.
export function buildInstrumentVoice(ctx: BaseAudioContext, dest: AudioNode, buf: AudioBuffer, hit: InstrumentHit, p: VoiceParams): BuiltVoice {
  const { shape } = p;
  const src = ctx.createBufferSource();
  src.buffer = buf;
  src.playbackRate.value = hit.rate;
  const offsetSec = Math.min(buf.duration, hit.offset / p.sourceRate);
  if (hit.loop) {
    src.loop = true;
    if (hit.loop !== 'whole') {
      src.loopStart = hit.loop.start / p.sourceRate;
      src.loopEnd = Math.min(buf.duration, hit.loop.end / p.sourceRate);
    }
  }

  const gain = ctx.createGain();
  const panner = ctx.createStereoPanner();
  panner.pan.value = Math.max(-1, Math.min(1, p.pan * 2 - 1 + hit.pan + shape.pan));

  let filter: BiquadFilterNode | null = null;
  const noteFilter = p.cutoffScale < 1 || p.resonanceAdd > 0;
  if (shape.filter || noteFilter) {
    const f = shape.filter ?? { type: 'lowpass' as const, cutoff: 20000, resonance: 0, envDepth: 0, envAttack: 0.001, envDecay: 0.001 };
    filter = ctx.createBiquadFilter();
    filter.type = f.type;
    const res = f.resonance + p.resonanceAdd;
    // Lowpass/highpass Q is already in dB; bandpass Q is linear.
    filter.Q.value = f.type === 'bandpass' ? Math.max(0.3, dbToGain(res)) : res;
    scheduleFilterEnv(filter.frequency, p.time, f.cutoff * p.cutoffScale, f.envDepth, f.envAttack, f.envDecay, ctx.sampleRate / 2);
    src.connect(gain).connect(filter).connect(panner).connect(dest);
  } else {
    src.connect(gain).connect(panner).connect(dest);
  }

  const peak = Math.max(0, Math.min(2, p.volume * hit.gain * dbToGain(shape.gainDb)));
  const release = shape.release * p.releaseScale;
  let end: number;
  if (hit.oneShot && !hit.loop) {
    // Rings out to the sample's end regardless of note length (drum hits).
    end = p.time + (buf.duration - offsetSec) / hit.rate;
    scheduleAdsr(gain.gain, p.time, null, peak, shape.attack, shape.decay, shape.sustain, 0);
    gain.gain.setValueAtTime(peak * shape.sustain, Math.max(p.time, end - 0.002));
    gain.gain.linearRampToValueAtTime(0, end);
  } else {
    end = scheduleAdsr(gain.gain, p.time, p.duration, peak, shape.attack, shape.decay, shape.sustain, release);
  }
  src.start(p.time, offsetSec);
  if (Number.isFinite(end)) src.stop(end + 0.01);
  return { src, gain, filter, panner, end, release };
}
