// Shared dB-scaling/coloring math for peak meters — extracted from
// PeakMeter.svelte (the toolbar's master stereo meter) so PeakMeterVertical
// (per-mixer-channel meters) can use the exact same scale/color rules
// instead of a second, potentially-diverging copy. See "Toolbar audio
// monitor & peak meter" in CLAUDE.md for the algorithm's rationale.

// Fast attack (jumps straight to a new, higher peak), slow release (decays
// by this factor per frame otherwise) — the classic VU-meter feel.
export const RELEASE = 0.85;

// dB range mapped onto the meter's 0..1 scale. 0dB = full digital scale
// (clipping) = the true top/right edge; anything at/below MIN_DB reads as
// empty.
export const MIN_DB = -48;

// Level-meter colors are semantic (safe → warning → clipping), so they stay
// green/yellow/red rather than following the peach --accent; softened to sit
// on the slate palette.
export const GREEN  = [141, 206, 178] as const;
export const YELLOW = [232, 200, 120] as const;
export const RED    = [229, 110, 110] as const;

export function peakOf(analyser: AnalyserNode, buf: Uint8Array): number {
  analyser.getByteTimeDomainData(buf);
  let peak = 0;
  for (let i = 0; i < buf.length; i++) {
    const dev = Math.abs(buf[i] - 128) / 128;
    if (dev > peak) peak = dev;
  }
  return peak;
}

export function ampToFraction(amp: number): number {
  if (amp <= 0) return 0;
  const db = 20 * Math.log10(amp);
  return Math.max(0, Math.min(1, (db - MIN_DB) / -MIN_DB));
}

export function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

// Green up to 75%, green->yellow up to 90%, yellow->red the rest of the
// way — stays calm through normal levels and only reddens as a channel
// actually approaches 0dBFS.
export function colorForLevel(frac: number): string {
  let c: readonly [number, number, number];
  if (frac < 0.75) {
    c = GREEN;
  } else if (frac < 0.9) {
    const t = (frac - 0.75) / 0.15;
    c = [lerp(GREEN[0], YELLOW[0], t), lerp(GREEN[1], YELLOW[1], t), lerp(GREEN[2], YELLOW[2], t)];
  } else {
    const t = Math.min(1, (frac - 0.9) / 0.1);
    c = [lerp(YELLOW[0], RED[0], t), lerp(YELLOW[1], RED[1], t), lerp(YELLOW[2], RED[2], t)];
  }
  return `rgb(${c[0]}, ${c[1]}, ${c[2]})`;
}
