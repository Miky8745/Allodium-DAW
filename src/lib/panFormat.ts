// Shared L##/C/R## pan display, used by ChannelRow's pan dial and the
// Mixer's per-track pan knob (both operate on the same 0..1, 0.5=center
// convention as ChannelData.pan / MixerChannelData.pan).
export function formatPan(v: number): string {
  if (v === 0.5) return 'C';
  return v < 0.5 ? `L${Math.round((0.5 - v) * 200)}` : `R${Math.round((v - 0.5) * 200)}`;
}
