import { pitchName } from '$lib/pianoroll/pitch';

export type MidiMessageType =
  | 'noteOn' | 'noteOff' | 'polyPressure' | 'cc' | 'program'
  | 'channelPressure' | 'pitchBend' | 'system';

export interface MidiMessage {
  type: MidiMessageType;
  channel: number; // 1-16 (0 for system messages)
  data1: number;   // note / controller / program
  data2: number;   // velocity / value
  bytes: number[];
}

const TYPES: Record<number, MidiMessageType> = {
  0x80: 'noteOff', 0x90: 'noteOn', 0xA0: 'polyPressure', 0xB0: 'cc',
  0xC0: 'program', 0xD0: 'channelPressure', 0xE0: 'pitchBend',
};

export function parseMidi(bytes: number[]): MidiMessage | null {
  if (bytes.length === 0) return null;
  const status = bytes[0];
  if (status < 0x80) return null; // running status isn't produced by midir
  if (status >= 0xF0) return { type: 'system', channel: 0, data1: bytes[1] ?? 0, data2: bytes[2] ?? 0, bytes };
  let type = TYPES[status & 0xF0];
  const data1 = bytes[1] ?? 0;
  const data2 = bytes[2] ?? 0;
  // Note-on with velocity 0 is a note-off by MIDI convention.
  if (type === 'noteOn' && data2 === 0) type = 'noteOff';
  return { type, channel: (status & 0x0F) + 1, data1, data2, bytes };
}

export function noteOnBytes(channel: number, pitch: number, velocity: number): number[] {
  return [0x90 | ((channel - 1) & 0x0F), pitch & 0x7F, Math.max(1, Math.min(127, Math.round(velocity)))];
}

export function noteOffBytes(channel: number, pitch: number): number[] {
  return [0x80 | ((channel - 1) & 0x0F), pitch & 0x7F, 0];
}

// Human-readable one-liner for the MIDI monitor log / toolbar LED hover.
export function describeMidi(msg: MidiMessage): string {
  switch (msg.type) {
    case 'noteOn':  return `Note on  ${pitchName(msg.data1)} vel ${msg.data2} · ch ${msg.channel}`;
    case 'noteOff': return `Note off ${pitchName(msg.data1)} · ch ${msg.channel}`;
    case 'cc':      return `CC ${msg.data1} = ${msg.data2} · ch ${msg.channel}`;
    case 'program': return `Program ${msg.data1} · ch ${msg.channel}`;
    case 'pitchBend': return `Pitch bend ${((msg.data2 << 7) | msg.data1) - 8192} · ch ${msg.channel}`;
    case 'channelPressure': return `Pressure ${msg.data1} · ch ${msg.channel}`;
    case 'polyPressure': return `Poly pressure ${pitchName(msg.data1)} = ${msg.data2} · ch ${msg.channel}`;
    default: return `System ${msg.bytes.map(b => b.toString(16).padStart(2, '0')).join(' ')}`;
  }
}
