// Piano-roll slide (pitch-bend) notes: a slide note never
// sounds itself; when it starts, every note of the *same channel* and the
// *same color* that's playing at that moment glides to the slide note's
// pitch, over the slide note's own length, and stays there until it ends.
// Consecutive slides chain (each glides on from wherever the last left off).
//
// Both sides arrive asynchronously and in either order: audioEngine
// registers a voice only once its sample has loaded (playSample), and a
// slide note in the same step may be scheduled before or after the note it
// bends. So slides are kept as timed events per channel, and whichever side
// arrives second applies them: addSlide() bends already-registered voices,
// registerVoice() picks up slides that were scheduled before it existed.

interface SlideEvent {
  id: number;
  time: number;       // AudioContext time the glide starts
  duration: number;   // seconds
  targetPitch: number; // absolute (fractional) semitones
}

interface Voice {
  key: string;
  param: AudioParam;  // the source's playbackRate
  pitch: number;      // sounding pitch at baseRate (fractional semitones)
  baseRate: number;
  start: number;
  end: number;        // nominal note end (note-off), not including the release tail
  events: SlideEvent[]; // applied slides, sorted by time
}

interface Segment { t0: number; t1: number; from: number; to: number }

const EPS = 1e-4;
// Slide events older than this (relative to the current time) can no
// longer affect any voice that's still to be registered.
const EVENT_TTL = 2;

const keyOf = (channelId: number, color: number) => `${channelId}:${color}`;

export class SlideRegistry {
  private voices = new Set<Voice>();
  private events = new Map<string, SlideEvent[]>();
  private nextEventId = 1;

  registerVoice(channelId: number, color: number, src: AudioBufferSourceNode, pitch: number, baseRate: number, start: number, end: number) {
    const key = keyOf(channelId, color);
    const voice: Voice = { key, param: src.playbackRate, pitch, baseRate, start, end, events: [] };
    const pending = (this.events.get(key) ?? []).filter(ev => this.covers(voice, ev.time));
    if (pending.length > 0) {
      voice.events = [...pending].sort((a, b) => a.time - b.time);
      this.reschedule(voice, voice.events[0].time);
    }
    this.voices.add(voice);
    src.addEventListener('ended', () => this.voices.delete(voice));
  }

  addSlide(channelId: number, color: number, time: number, duration: number, targetPitch: number, now: number) {
    const key = keyOf(channelId, color);
    const ev: SlideEvent = { id: this.nextEventId++, time, duration: Math.max(0, duration), targetPitch };
    const list = (this.events.get(key) ?? []).filter(e => e.time + e.duration > now - EVENT_TTL);
    list.push(ev);
    this.events.set(key, list);
    for (const voice of this.voices) {
      if (voice.key !== key || !this.covers(voice, time)) continue;
      voice.events.push(ev);
      voice.events.sort((a, b) => a.time - b.time);
      this.reschedule(voice, time);
    }
  }

  // Forget pending slides (transport stop). Voices already sounding keep
  // whatever automation they were given.
  clear() {
    this.events.clear();
  }

  // "Currently playing" = started at/before the slide and not yet released.
  private covers(voice: Voice, time: number): boolean {
    return time >= voice.start - EPS && time < voice.end - EPS;
  }

  private rateFor(voice: Voice, pitch: number): number {
    return voice.baseRate * 2 ** ((pitch - voice.pitch) / 12);
  }

  // Glide segments in rate space. Each segment is cut short where the next
  // slide begins, so automation ramps never overlap.
  private segments(voice: Voice): Segment[] {
    const segs: Segment[] = [];
    let current = voice.baseRate;
    voice.events.forEach((ev, i) => {
      const to = this.rateFor(voice, ev.targetPitch);
      const fullEnd = ev.time + ev.duration;
      const next = voice.events[i + 1];
      const t1 = next ? Math.min(fullEnd, next.time) : fullEnd;
      const seg: Segment = { t0: ev.time, t1, from: current, to };
      if (t1 < fullEnd) seg.to = interp({ ...seg, t1: fullEnd }, t1);
      segs.push(seg);
      current = seg.to;
    });
    return segs;
  }

  // Rewrites the voice's playbackRate automation from `fromTime` onward.
  // A glide already in progress at `fromTime` has its ramp end event
  // cancelled too, so it's re-added ending at its (now truncated) end —
  // exponential interpolation is self-similar, so the curve up to that
  // point is unchanged.
  private reschedule(voice: Voice, fromTime: number) {
    const p = voice.param;
    p.cancelScheduledValues(fromTime);
    for (const seg of this.segments(voice)) {
      if (seg.t1 <= fromTime - EPS) continue;
      if (seg.t0 < fromTime - EPS) {
        p.exponentialRampToValueAtTime(seg.to, seg.t1);
      } else if (seg.t1 - seg.t0 < 0.001) {
        p.setValueAtTime(seg.to, seg.t0);
      } else {
        p.setValueAtTime(seg.from, seg.t0);
        p.exponentialRampToValueAtTime(seg.to, seg.t1);
      }
    }
  }
}

function interp(seg: Segment, t: number): number {
  if (t >= seg.t1) return seg.to;
  if (t <= seg.t0) return seg.from;
  const k = (t - seg.t0) / (seg.t1 - seg.t0);
  return seg.from * (seg.to / seg.from) ** k;
}
