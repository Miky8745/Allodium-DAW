// AudioParam automation for Bloom's amp and filter envelopes. Everything is
// scheduled up front (the scheduler knows each note's length), except held
// live voices, which get their release from releaseParam() on key-up.

// Linear ADSR level `x` seconds into a note that hasn't been released yet.
function levelAt(x: number, peak: number, a: number, d: number, s: number): number {
  if (x < a) return peak * (x / a);
  if (x < a + d) return peak + (peak * s - peak) * ((x - a) / d);
  return peak * s;
}

// `dur` = note length in seconds (release starts there), or null for a held
// voice (attack/decay into sustain, release left to releaseParam()). A note
// shorter than its attack/decay releases from wherever the envelope got to.
// Returns when the voice is silent (Infinity for a held voice).
export function scheduleAdsr(param: AudioParam, t: number, dur: number | null, peak: number, a: number, d: number, s: number, r: number): number {
  param.setValueAtTime(0, t);
  const until = dur ?? Infinity;
  if (until <= a) {
    param.linearRampToValueAtTime(levelAt(until, peak, a, d, s), t + until);
  } else {
    param.linearRampToValueAtTime(peak, t + a);
    if (until <= a + d) {
      param.linearRampToValueAtTime(levelAt(until, peak, a, d, s), t + until);
    } else {
      param.linearRampToValueAtTime(peak * s, t + a + d);
      if (dur !== null) param.setValueAtTime(peak * s, t + dur);
    }
  }
  if (dur === null) return Infinity;
  param.linearRampToValueAtTime(0, t + dur + r);
  return t + dur + r;
}

// Releases a held param from whatever value it has right now.
export function releaseParam(param: AudioParam, now: number, r: number): void {
  const p = param as AudioParam & { cancelAndHoldAtTime?: (t: number) => AudioParam };
  if (p.cancelAndHoldAtTime) {
    p.cancelAndHoldAtTime(now);
  } else {
    param.cancelScheduledValues(now);
    param.setValueAtTime(param.value, now);
  }
  param.linearRampToValueAtTime(0, now + r);
}

// Filter envelope: base cutoff → base·2^(depth/1200) over `a`, then back to
// base over `d` (sustain is always the base cutoff).
export function scheduleFilterEnv(param: AudioParam, t: number, base: number, depthCents: number, a: number, d: number, nyquist: number): void {
  const clampHz = (hz: number) => Math.max(20, Math.min(nyquist, hz));
  const from = clampHz(base);
  param.setValueAtTime(from, t);
  if (depthCents === 0) return;
  param.exponentialRampToValueAtTime(clampHz(base * 2 ** (depthCents / 1200)), t + a);
  param.exponentialRampToValueAtTime(from, t + a + d);
}
