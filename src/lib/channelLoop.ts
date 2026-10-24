import type { ChannelData } from './types';

// How many steps of a channel's own content repeat before it restarts,
// independent of the overall loop bound. Finds the last bar containing any
// content for this channel, then loops at that bar's end — which is exactly
// the point where "there are no notes in the next bar" holds for every step
// from then to the end of loopLength. A channel with content reaching all
// the way to loopLength's end has no such trailing gap, so it just plays
// through once.
export function channelLoopLength(ch: ChannelData, loopLength: number, barLength: number): number {
  const bars = Math.ceil(loopLength / barLength);
  let lastOccupiedBar = -1;
  for (let bar = 0; bar < bars; bar++) {
    const from = bar * barLength;
    const to   = Math.min(from + barLength, loopLength);
    const occupied = ch.notes.length > 0
      ? ch.notes.some(n => n.start < to && n.start + n.length > from)
      : stepsOccupied(ch.steps, from, to);
    if (occupied) lastOccupiedBar = bar;
  }
  if (lastOccupiedBar === -1) return loopLength; // no content — loop length is moot
  return Math.min((lastOccupiedBar + 1) * barLength, loopLength);
}

// Whether any step in [from, to) is on — an index loop rather than
// slice().some(): this runs for every channel on every scheduled step, and
// the throwaway arrays add up to GC pauses during playback.
function stepsOccupied(steps: boolean[], from: number, to: number): boolean {
  for (let i = from; i < to; i++) if (steps[i]) return true;
  return false;
}

// How far one channel's own content reaches, in steps, rounded up to a whole
// bar, with no external bound — 0 if the channel has no content at all.
// Used to size a placed pattern's Song-mode block to its content (see
// patternContentLength below) rather than to an arbitrary manually-set
// length.
export function channelContentLength(ch: ChannelData, barLength: number): number {
  if (ch.notes.length > 0) {
    let end = 0;
    for (const n of ch.notes) end = Math.max(end, n.start + n.length);
    return Math.ceil(end / barLength) * barLength;
  }
  const lastIdx = ch.steps.reduce((last, v, i) => (v ? i : last), -1);
  if (lastIdx === -1) return 0;
  return Math.ceil((lastIdx + 1) / barLength) * barLength;
}

// The longest channel's own content reach across a whole pattern's channels
// — "as long as the longest channel in the pattern". Floored at one bar so
// an empty pattern still has some extent to place on the Song-mode timeline.
export function patternContentLength(channels: ChannelData[], barLength: number): number {
  let max = 0;
  for (const ch of channels) max = Math.max(max, channelContentLength(ch, barLength));
  return max > 0 ? max : barLength;
}
