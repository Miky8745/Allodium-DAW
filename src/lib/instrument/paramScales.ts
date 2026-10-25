// Knob scales for the instrument window: each maps a real parameter value
// to the 0..1 travel of a Dial and back, plus its readout format.
export interface ParamScale {
  toNorm: (v: number) => number;
  fromNorm: (n: number) => number;
  format: (v: number) => string;
}

const clamp01 = (n: number) => Math.max(0, Math.min(1, n));

function expScale(min: number, max: number, format: (v: number) => string): ParamScale {
  const ratio = Math.log(max / min);
  return {
    toNorm: v => clamp01(Math.log(Math.max(min, v) / min) / ratio),
    fromNorm: n => min * Math.exp(clamp01(n) * ratio),
    format,
  };
}

function linScale(min: number, max: number, format: (v: number) => string, step = 0): ParamScale {
  return {
    toNorm: v => clamp01((v - min) / (max - min)),
    fromNorm: n => {
      const v = min + clamp01(n) * (max - min);
      return step ? Math.round(v / step) * step : v;
    },
    format,
  };
}

export const formatSeconds = (s: number) => (s < 1 ? `${Math.round(s * 1000)} ms` : `${s.toFixed(2)} s`);
export const formatHz = (hz: number) => (hz < 1000 ? `${Math.round(hz)} Hz` : `${(hz / 1000).toFixed(1)} kHz`);

export const TIME = expScale(0.001, 10, formatSeconds);
export const LEVEL = linScale(0, 1, v => `${Math.round(v * 100)}%`);
export const CUTOFF = expScale(20, 20000, formatHz);
export const RESONANCE = linScale(0, 24, v => `${v.toFixed(1)} dB`);
export const ENV_DEPTH = linScale(-9600, 9600, v => `${v >= 0 ? '+' : ''}${(v / 1200).toFixed(1)} oct`);
export const TUNE = linScale(-24, 24, v => `${v > 0 ? '+' : ''}${Math.round(v)} st`, 1);
export const MACRO = linScale(0, 1, v => `${Math.round(v * 100)}%`);
