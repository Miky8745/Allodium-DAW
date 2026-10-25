// Sample rate of an encoded audio file, read from its header before
// decodeAudioData() resamples it to the context rate. SFZ loop points and
// offsets are in *source* frames, so converting them to seconds needs this.
// Handles WAV, FLAC and Ogg Vorbis/Opus; null for anything else (callers
// fall back to the decoded buffer's rate).
export function sourceSampleRate(bytes: ArrayBuffer): number | null {
  const b = new Uint8Array(bytes);
  const dv = new DataView(bytes);
  const ascii = (at: number, len: number) => String.fromCharCode(...b.subarray(at, at + len));
  if (b.length < 32) return null;

  if (ascii(0, 4) === 'RIFF' && ascii(8, 4) === 'WAVE') {
    let at = 12;
    while (at + 8 <= b.length) {
      const size = dv.getUint32(at + 4, true);
      if (ascii(at, 4) === 'fmt ' && at + 16 <= b.length) return dv.getUint32(at + 12, true);
      at += 8 + size + (size & 1);
    }
    return null;
  }

  if (ascii(0, 4) === 'fLaC') {
    // STREAMINFO is always the first metadata block; the rate is its
    // 20 bits starting at byte 10 of the block body.
    const s = 8 + 10;
    return (b[s] << 12) | (b[s + 1] << 4) | (b[s + 2] >> 4);
  }

  if (ascii(0, 4) === 'OggS') {
    const packet = 27 + b[26]; // page header + segment table
    if (packet + 16 > b.length) return null;
    if (b[packet] === 1 && ascii(packet + 1, 6) === 'vorbis') return dv.getUint32(packet + 12, true);
    if (ascii(packet, 8) === 'OpusHead') return 48000; // Opus always decodes at 48 kHz
  }
  return null;
}
