// System-audio source for the Toolbar's audio monitor (BlackBox, when
// Preferences > General > "Audio monitor source" is "System audio"). Rust
// (capture.rs) records the default output's monitor and streams mono f32
// chunks over a binary Channel; a tiny AudioWorklet replays them into an
// AnalyserNode so BlackBox reads it exactly like the master-bus analyser.
// The analyser reaches the destination only through a zero gain — the graph
// has to be pulled to run, but nothing is ever audible.
import { Channel, invoke } from '@tauri-apps/api/core';
import { audioEngine } from './audioEngine';

// Ring buffer in the worklet: drops the oldest audio once more than ~85ms
// (4096 samples at 48kHz) is queued, so the display can't drift behind what
// you hear; outputs silence when it runs dry.
const WORKLET_SRC = `
class SystemAudioFeed extends AudioWorkletProcessor {
  constructor() {
    super();
    this.buf = new Float32Array(16384);
    this.read = 0;
    this.size = 0;
    this.port.onmessage = (e) => {
      const data = e.data;
      const cap = this.buf.length;
      for (let i = 0; i < data.length; i++) {
        this.buf[(this.read + this.size) % cap] = data[i];
        if (this.size < cap) this.size++; else this.read = (this.read + 1) % cap;
      }
      const MAX = 4096;
      if (this.size > MAX) { this.read = (this.read + this.size - MAX) % cap; this.size = MAX; }
    };
  }
  process(_inputs, outputs) {
    const out = outputs[0][0];
    const cap = this.buf.length;
    for (let i = 0; i < out.length; i++) {
      if (this.size > 0) { out[i] = this.buf[this.read]; this.read = (this.read + 1) % cap; this.size--; }
      else out[i] = 0;
    }
    return true;
  }
}
registerProcessor('system-audio-feed', SystemAudioFeed);
`;

class SystemAudio {
  private analyser: AnalyserNode | null = null;
  private feed: AudioWorkletNode | null = null;
  private sink: GainNode | null = null;
  private moduleCtx: AudioContext | null = null;
  private wanted = false;
  // Bumped by every start()/stop() so a slow async start that's been
  // superseded doesn't wire itself up afterwards.
  private generation = 0;
  private unsubRebuild: (() => void) | null = null;

  getAnalyser(): AnalyserNode | null {
    return this.analyser;
  }

  async start(): Promise<void> {
    this.wanted = true;
    this.unsubRebuild ??= audioEngine.onContextRebuilt(() => {
      // Old nodes belong to the closed context — rebuild at the new rate.
      if (this.wanted) void this.start();
    });
    const gen = ++this.generation;
    this.teardownNodes();
    try {
      const ctx = await audioEngine.getRunningContext();
      if (this.moduleCtx !== ctx) {
        const url = URL.createObjectURL(new Blob([WORKLET_SRC], { type: 'application/javascript' }));
        try { await ctx.audioWorklet.addModule(url); } finally { URL.revokeObjectURL(url); }
        this.moduleCtx = ctx;
      }
      if (gen !== this.generation) return;

      const feed = new AudioWorkletNode(ctx, 'system-audio-feed', { numberOfInputs: 0, outputChannelCount: [1] });
      const analyser = ctx.createAnalyser();
      analyser.fftSize = 2048;
      analyser.smoothingTimeConstant = 0.75;
      const sink = ctx.createGain();
      sink.gain.value = 0;
      feed.connect(analyser);
      analyser.connect(sink);
      sink.connect(ctx.destination);

      const channel = new Channel<ArrayBuffer | number[]>();
      channel.onmessage = (buf) => {
        if (gen !== this.generation) return;
        const bytes = buf instanceof ArrayBuffer ? buf : new Uint8Array(buf).buffer;
        feed.port.postMessage(new Float32Array(bytes));
      };
      await invoke('system_audio_start', { sampleRate: ctx.sampleRate, onData: channel });
      if (gen !== this.generation) {
        feed.disconnect(); analyser.disconnect(); sink.disconnect();
        return;
      }
      this.feed = feed;
      this.analyser = analyser;
      this.sink = sink;
    } catch (e) {
      console.error('System audio capture failed', e);
    }
  }

  stop(): void {
    this.wanted = false;
    this.generation++;
    this.unsubRebuild?.();
    this.unsubRebuild = null;
    this.teardownNodes();
    invoke('system_audio_stop').catch(() => {});
  }

  private teardownNodes(): void {
    this.feed?.disconnect();
    this.analyser?.disconnect();
    this.sink?.disconnect();
    this.feed = this.analyser = this.sink = null;
  }
}

export const systemAudio = new SystemAudio();
