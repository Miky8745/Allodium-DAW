// Hex/HSV color helpers backing ColorWheel.svelte and ChannelRow's
// right-click "Recolor randomly". No color-picker library exists anywhere
// in this project (checked package.json) — hand-rolled, matching every
// other control in the app (Dial, Slider, ScrollField are all built from
// scratch too).

export function hexToRgb(hex: string): [number, number, number] {
  const clean = hex.replace('#', '');
  const r = parseInt(clean.slice(0, 2), 16) || 0;
  const g = parseInt(clean.slice(2, 4), 16) || 0;
  const b = parseInt(clean.slice(4, 6), 16) || 0;
  return [r, g, b];
}

function toHex2(n: number): string {
  return Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, '0');
}

export function rgbToHex(r: number, g: number, b: number): string {
  return `#${toHex2(r)}${toHex2(g)}${toHex2(b)}`;
}

// h: 0..360, s/v: 0..1
export function hsvToRgb(h: number, s: number, v: number): [number, number, number] {
  const c = v * s;
  const hp = (((h % 360) + 360) % 360) / 60;
  const x = c * (1 - Math.abs((hp % 2) - 1));
  let r = 0, g = 0, b = 0;
  if (hp < 1) [r, g, b] = [c, x, 0];
  else if (hp < 2) [r, g, b] = [x, c, 0];
  else if (hp < 3) [r, g, b] = [0, c, x];
  else if (hp < 4) [r, g, b] = [0, x, c];
  else if (hp < 5) [r, g, b] = [x, 0, c];
  else [r, g, b] = [c, 0, x];
  const m = v - c;
  return [(r + m) * 255, (g + m) * 255, (b + m) * 255];
}

export function rgbToHsv(r: number, g: number, b: number): [number, number, number] {
  const rn = r / 255, gn = g / 255, bn = b / 255;
  const max = Math.max(rn, gn, bn), min = Math.min(rn, gn, bn);
  const d = max - min;
  let h = 0;
  if (d !== 0) {
    if (max === rn) h = 60 * (((gn - bn) / d) % 6);
    else if (max === gn) h = 60 * ((bn - rn) / d + 2);
    else h = 60 * ((rn - gn) / d + 4);
  }
  if (h < 0) h += 360;
  const s = max === 0 ? 0 : d / max;
  const v = max;
  return [h, s, v];
}

// "A color" per the user's own definition: at least one channel-pair gap of
// 50 (0-255 range) — i.e. not a washed-out near-grey.
export function isColorful(r: number, g: number, b: number, minSpread = 50): boolean {
  return Math.max(r, g, b) - Math.min(r, g, b) >= minSpread;
}

// Random hue with saturation/value high enough that the resulting RGB
// spread is virtually guaranteed to clear isColorful()'s threshold, with a
// resample loop as a safety net for the rare edge case near a hue boundary.
export function randomColorfulHex(): string {
  for (let attempt = 0; attempt < 10; attempt++) {
    const h = Math.random() * 360;
    const s = 0.65 + Math.random() * 0.35; // 0.65..1
    const v = 0.7 + Math.random() * 0.3;   // 0.7..1
    const [r, g, b] = hsvToRgb(h, s, v);
    if (isColorful(r, g, b)) return rgbToHex(r, g, b);
  }
  // Fallback: pure hue at full saturation/value always clears the threshold.
  const [r, g, b] = hsvToRgb(Math.random() * 360, 1, 1);
  return rgbToHex(r, g, b);
}

// Legible text color for content drawn on top of an arbitrary fill (same
// luminance threshold ChannelRow uses for its recolored sample box).
export function readableTextOn(hex: string): string {
  const [r, g, b] = hexToRgb(hex);
  const luminance = (r * 299 + g * 587 + b * 114) / 1000;
  return luminance < 140 ? 'rgba(255,255,255,0.88)' : 'rgba(0,0,0,0.6)';
}
