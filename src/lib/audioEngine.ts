import { invoke } from '@tauri-apps/api/core';
import type { ChannelData } from '$lib/types';
import { buildMultisample, pickFromFolder, type MultisampleFolder } from '$lib/multisample';
import { channelLoopLength } from '$lib/channelLoop';
import { MASTER_TRACK_ID } from '$lib/mixerStore.svelte';
import type { SongPlacement } from '$lib/songTimeline.svelte';
import { SlideRegistry } from '$lib/slideVoices';
import { profiler } from '$lib/debug/profiler';
import { soundingPitch, type NoteProps } from '$lib/noteProps';

// Scheduler look-ahead (seconds) / tick interval (ms) defaults — the live
// values are instance fields set from Preferences > General via
// setSchedulerTiming().
const DEFAULT_LOOKAHEAD = 0.1;
const DEFAULT_INTERVAL  = 25;
const RAMP      = 0.0005; // 0.5ms de-click ramp — short enough not to affect transients
// How late loadSample() is allowed to resolve past a hit's intended playTime
// before scheduleHit() drops it — guards the scheduler against an audible
// pile-up of stale beats. Widened from an earlier 50ms: a cold IPC read +
// decodeAudioData() routinely exceeds that on the very first trigger of a
// given sample, which silently dropped the hit entirely instead of playing
// it. previewNote() bypasses this cutoff altogether (see allowLate below)
// since a one-shot audition has no pile-up concern.
const STALE_CUTOFF_SEC = 0.15;

// Fixed ADSR envelope applied to piano-roll notes (not plain step-sequencer
// one-shots) so a note actually stops when its length says it should,
// instead of ringing out to the sample's full recorded duration.
const ATTACK  = 0.001;     // samples already carry their own natural attack
const DECAY   = 0.05;  // 50ms
const SUSTAIN = 0.7;   // sustain level, fraction of peak volume
const RELEASE = 0.05;  // 50ms

// Note "rel" property (0..1) → release seconds. 0.5 is exactly RELEASE (the
// pre-properties behavior); below shortens it toward a near-instant cut,
// above stretches it up to 2s, ringing out past the note's end.
function releaseSeconds(rel: number): number {
  return rel <= 0.5 ? Math.max(0.002, RELEASE * rel * 2) : RELEASE + (rel - 0.5) * 2 * 1.95;
}

// Note "mod X"/"mod Y" (0..1) drive a per-note lowpass filter, the usual
// sampler mapping: X = cutoff (exponential 50Hz..20kHz, 1 = open),
// Y = resonance. A note at X=1/Y=0 (the defaults) gets no filter node.
const filterCutoff = (modX: number) => 50 * 400 ** Math.max(0, Math.min(1, modX));
const filterQ = (modY: number) => 0.707 + Math.max(0, Math.min(1, modY)) ** 2 * 18;

// Disconnects a finished voice's nodes (null entries skipped).
function disconnectAll(...nodes: (AudioNode | null)[]) {
  for (const n of nodes) n?.disconnect();
}

// Extra per-voice data for a piano-roll note, threaded from
// scheduleChannelStep()/previewNote() down to playSample(). `pitch` is the
// note's sounding pitch (incl. fine pitch) — what the slide registry bends
// relative to.
interface NoteVoiceOpts {
  channelId: number;
  color: number;
  pitch: number;
  release: number;
  modX: number;
  modY: number;
}

function voiceOpts(ch: ChannelData, props: NoteProps, pitch: number): NoteVoiceOpts {
  return { channelId: ch.id, color: props.color, pitch, release: props.release, modX: props.modX, modY: props.modY };
}

// Note pan is an offset on top of the channel's own pan (0.5 = no change).
const combinePan = (chPan: number, notePan: number) => Math.max(0, Math.min(1, chPan + notePan - 0.5));

// One-off audition sound for the Piano Roll (click/place/move a note) —
// short so rapid pitch-row sweeps during a drag don't pile up into mud.
// Defaults only; the live values come from Preferences > General via
// setPreview().
const DEFAULT_PREVIEW_DURATION = 0.15;
const DEFAULT_PREVIEW_VELOCITY = 0.8; // matches NoteGrid's createNote() default note velocity

// Toolbar's metronome — two pre-recorded clicks shipped under data/samples/
// (data-relative paths, same convention as ChannelData.samplePath), rather
// than a synthesized OscillatorNode tone, so it reuses the exact same
// loadSample()/playSample() pipeline (caching, de-click ramps, etc.) as
// every other sample in the app instead of a one-off second sound path.
// metronome02 accents beat 1 of every bar; metronome01 is the plain click
// on every other quarter note (swapped from the initial pairing — 02 reads
// as the stronger/more distinct hit of the two).
const METRONOME_ACCENT_PATH = 'samples/misc/metronome02.ogg';
const METRONOME_CLICK_PATH  = 'samples/misc/metronome01.ogg';
const METRONOME_VOLUME = 0.9;

// Per-mixer-track node chain, one per mixerStore channel (keyed by
// MixerChannelData.id) plus one implicit for Master. A triggered sample's
// per-hit chain (gain + panner, see playSample()) connects into `input`
// instead of straight to the master bus, so the mixer's fader/pan/mute-solo
// controls (see mixerAudio.svelte.ts) actually affect what's routed there.
// Order: input(gain=volume) -> pan -> mute(gain=0|1), which fans out to
// `analyser` (a parallel tap, never inline — same convention as the
// existing master-bus taps in buildCtx()) and onward to wherever `routeTo`
// currently points (another chain's `input`, or masterVolumeGain).
interface TrackChain {
  input: GainNode;
  pan: StereoPannerNode;
  mute: GainNode;
  analyser: AnalyserNode;
}

// A held live note (MIDI input / QWERTY keyboard), started by
// startLiveVoice() and sustained until releaseLiveVoice(). `src`/`gain` stay
// unset until the sample has loaded; a release arriving before that just
// marks it `released` so it never starts at all. `bend` (semitones) is kept
// even before the source exists, so a bend arriving mid-load still applies.
interface LiveVoice {
  src?: AudioBufferSourceNode;
  gain?: GainNode;
  baseRate: number;
  bend: number;
  released: boolean;
}

// Smoothing time constant for pitch-bend rate changes — long enough to hide
// the 14-bit/message-rate stepping, short enough to feel immediate.
const BEND_SMOOTHING = 0.008;

export interface AudioContextOptions {
  latencyHint: AudioContextLatencyCategory;
  sampleRate: number; // 0 = device default
}

// Called for every piano-roll note the scheduler (or a preview) triggers —
// how midiOutput.ts mirrors those notes out as MIDI without this engine
// importing any stores. `velocity` is the note's own 0..1 velocity (not
// multiplied by channel volume); `playTime` is AudioContext time.
export type NoteSink = (ch: ChannelData, pitch: number, velocity: number, playTime: number, durationSeconds: number) => void;

class AudioEngine {
  private ctx: AudioContext | null = null;
  private masterOut: AudioNode | null = null;
  // Sits upstream of the existing masterOut compressor — every track chain
  // routed (directly or transitively) to Master converges here, and the
  // Mixer's Master fader controls this node's gain. Inserted without
  // disturbing masterOut's existing downstream wiring (destination +
  // analyser taps), which stays fed from the compressor exactly as before.
  private masterVolumeGain: GainNode | null = null;
  private trackChains = new Map<number, TrackChain>();
  // Tap off the master bus for visualization (Toolbar's blackbox monitor +
  // stereo peak meter). Pure analysis nodes — never connected onward to
  // destination themselves, so adding/removing them can't affect audible
  // output. `analyser` sees the full mixed signal (used for the
  // oscilloscope/spectrum); `analyserL`/`analyserR` sit behind a splitter for
  // the two independent peak-meter bars.
  private analyser: AnalyserNode | null = null;
  private analyserL: AnalyserNode | null = null;
  private analyserR: AnalyserNode | null = null;
  private timerId: ReturnType<typeof setTimeout> | null = null;
  private scheduledStep  = 0;
  private nextStepTime   = 0;
  private getBpm:          () => number         = () => 130;
  private getChannels:     () => ChannelData[]  = () => [];
  private getBarLength:    () => number         = () => 16;
  private getActiveLength: () => number         = () => 16;
  private getMetronomeOn:  () => boolean        = () => false;
  // Channel Rack solo (Ctrl+click a channel's mute dot in ChannelRow —
  // unrelated to the Mixer's own separate soloed field). A live getter
  // rather than importing channelStore directly, matching every other
  // live-value dependency here (getBpm/getChannels/getBarLength/…).
  private getAnySoloed:    () => boolean        = () => false;
  // 'pattern': loops channelStore's currently-active pattern forever (today's
  // original behavior, driven by getChannels/getActiveLength). 'song': plays
  // the Playlist arrangement once and stops at its end — see start()/
  // startSong() and tick()'s branching below.
  private mode: 'pattern' | 'song' = 'pattern';
  private getSongPlacements:  () => SongPlacement[]              = () => [];
  private getPatternChannels: (patternId: number) => ChannelData[] = () => [];
  private getSongLength:      () => number                       = () => 16;
  // Called once, from within tick(), the moment song-mode playback reaches
  // the end of the arrangement — lets playbackStore flip isPlaying off, the
  // same end state as pressing Stop. Never called in pattern mode (which
  // loops forever, unchanged).
  private onSongEnd: () => void = () => {};
  private bufferCache   = new Map<string, AudioBuffer>();
  // Deduplicates concurrent loads: multiple ticks requesting the same
  // uncached path share one IPC call instead of firing N in parallel.
  private loadingPromises = new Map<string, Promise<AudioBuffer | null>>();
  // Same dedup shape as bufferCache/loadingPromises above, but for a folder's
  // file listing (used to build a channel's multisample map the first time
  // it's played from).
  private folderCache   = new Map<string, MultisampleFolder>();
  private folderLoadingPromises = new Map<string, Promise<MultisampleFolder | null>>();

  stepDuration   = 0.5; // always kept current in tick()
  // Audio-context time the current loop cycle began. Advanced forward by
  // exactly one cycle each time real time crosses one (see getPosition())
  // rather than staying fixed at playback start, so a growing activeLength
  // (e.g. auto-extending the pattern while drawing notes) can't retroactively
  // change where "elapsed time so far" lands under a new, larger divisor.
  private loopAnchorTime = 0;

  private lookahead = DEFAULT_LOOKAHEAD;
  private interval  = DEFAULT_INTERVAL;
  private previewDuration = DEFAULT_PREVIEW_DURATION;
  private previewVelocity = DEFAULT_PREVIEW_VELOCITY;
  private ctxOptions: AudioContextOptions = { latencyHint: 'interactive', sampleRate: 0 };
  private liveVoices = new Map<number, LiveVoice>();
  private nextVoiceId = 1;
  private noteSink: NoteSink | null = null;
  private stopHook: (() => void) | null = null;
  private rebuildListeners = new Set<() => void>();
  // Piano-roll slide notes — see slideVoices.ts.
  private slides = new SlideRegistry();

  private buildCtx(): AudioContext {
    const ctx = new AudioContext({
      latencyHint: this.ctxOptions.latencyHint,
      ...(this.ctxOptions.sampleRate > 0 ? { sampleRate: this.ctxOptions.sampleRate } : {}),
    });
    // Limiter: only acts near 0 dBFS so it prevents clipping without
    // coloring the sound during normal single-channel use.
    const comp = ctx.createDynamicsCompressor();
    comp.threshold.value = -6;
    comp.knee.value      = 0;
    comp.ratio.value     = 20;
    comp.attack.value    = 0.001;
    comp.release.value   = 0.05;
    comp.connect(ctx.destination);
    this.masterOut = comp;

    // Every track chain routed to Master feeds this gain node rather than
    // `comp` directly — the Mixer's Master fader controls it (see
    // setMasterVolume()). comp's own downstream wiring (destination +
    // analyser taps below) is unchanged.
    const masterVolumeGain = ctx.createGain();
    masterVolumeGain.connect(comp);
    this.masterVolumeGain = masterVolumeGain;

    const analyser = ctx.createAnalyser();
    analyser.fftSize = 2048;
    analyser.smoothingTimeConstant = 0.75;
    comp.connect(analyser);
    this.analyser = analyser;

    const splitter = ctx.createChannelSplitter(2);
    comp.connect(splitter);
    const analyserL = ctx.createAnalyser();
    const analyserR = ctx.createAnalyser();
    analyserL.fftSize = 1024;
    analyserR.fftSize = 1024;
    splitter.connect(analyserL, 0);
    splitter.connect(analyserR, 1);
    this.analyserL = analyserL;
    this.analyserR = analyserR;

    return ctx;
  }

  // Full mixed-down master signal, for the Toolbar blackbox's oscilloscope
  // (getByteTimeDomainData) / spectrum (getByteFrequencyData) modes. Null
  // until the AudioContext has been built (warmUp()/ensureCtx()).
  getAnalyser(): AnalyserNode | null {
    return this.analyser;
  }

  // The (resumed) AudioContext, for systemAudio.ts to hang its capture
  // worklet + analyser off. Rebuilt contexts are announced via
  // onContextRebuilt().
  getRunningContext(): Promise<AudioContext> {
    return this.ensureCtx();
  }

  // Independent left/right taps for the Toolbar's stereo peak meter. Null
  // until the AudioContext has been built.
  getStereoAnalysers(): { left: AnalyserNode; right: AnalyserNode } | null {
    if (!this.analyserL || !this.analyserR) return null;
    return { left: this.analyserL, right: this.analyserR };
  }

  // Idempotent create-if-absent for a mixer track's node chain. Lazily
  // builds the AudioContext if needed (same "safe anytime" contract as
  // loadSample()/previewNote()) since mixerAudio.svelte.ts's bridge wants to
  // pre-populate every mixerStore channel's chain at project-open time,
  // possibly before warmUp() has ever run. Starts with no outgoing routing
  // edges at all — mixerAudio.svelte.ts's routing-diff effect is what
  // connects it (to Master by default, per mixerStore's own default
  // routeTo), since routing is now a multi-target fan-out the store owns,
  // not something this engine should default on its own.
  ensureTrackChain(trackId: number): TrackChain {
    const existing = this.trackChains.get(trackId);
    if (existing) return existing;
    if (!this.ctx) this.ctx = this.buildCtx();
    const ctx = this.ctx;
    const input = ctx.createGain();
    const pan = ctx.createStereoPanner();
    const mute = ctx.createGain();
    const analyser = ctx.createAnalyser();
    analyser.fftSize = 1024;
    input.connect(pan).connect(mute);
    mute.connect(analyser); // parallel meter tap — always connected, independent of routing
    const chain: TrackChain = { input, pan, mute, analyser };
    this.trackChains.set(trackId, chain);
    return chain;
  }

  removeTrackChain(trackId: number): void {
    const chain = this.trackChains.get(trackId);
    if (!chain) return;
    chain.input.disconnect();
    chain.pan.disconnect();
    chain.mute.disconnect();
    this.trackChains.delete(trackId);
  }

  setTrackVolume(trackId: number, volume: number): void {
    const chain = this.ensureTrackChain(trackId);
    chain.input.gain.value = Math.max(0, Math.min(2, volume));
  }

  setTrackPan(trackId: number, pan: number): void {
    const chain = this.ensureTrackChain(trackId);
    chain.pan.pan.value = pan * 2 - 1; // 0..1 -> -1..1
  }

  setTrackAudible(trackId: number, audible: boolean): void {
    const chain = this.ensureTrackChain(trackId);
    chain.mute.gain.value = audible ? 1 : 0;
  }

  // Adds one fan-out edge to a track's routing (a track can route to
  // several targets at once — sidechaining/multi-bus sends). Web Audio
  // natively supports one output connecting to many destinations, so this
  // is just another .connect() call; the analyser tap (a separate parallel
  // connection, see ensureTrackChain()) is untouched.
  connectTrackRoute(trackId: number, targetTrackId: number): void {
    const chain = this.ensureTrackChain(trackId);
    const target = targetTrackId === MASTER_TRACK_ID
      ? this.masterVolumeGain!
      : this.ensureTrackChain(targetTrackId).input;
    chain.mute.connect(target);
  }

  // Removes one fan-out edge. Selective disconnect (AudioNode.disconnect
  // with a destination argument) so other edges from the same track, and
  // the always-on analyser tap, are left alone. A no-op if either side's
  // chain doesn't exist (e.g. the target track was already removed —
  // removeTrackChain() already tore down its input node, which drops the
  // edge on that end regardless).
  disconnectTrackRoute(trackId: number, targetTrackId: number): void {
    const chain = this.trackChains.get(trackId);
    if (!chain) return;
    const target = targetTrackId === MASTER_TRACK_ID
      ? this.masterVolumeGain
      : this.trackChains.get(targetTrackId)?.input;
    if (target) chain.mute.disconnect(target);
  }

  setMasterVolume(volume: number): void {
    if (!this.masterVolumeGain) return;
    this.masterVolumeGain.gain.value = Math.max(0, Math.min(2, volume));
  }

  // For PeakMeterVertical — null until the track's chain has been created
  // (mixerAudio.svelte.ts pre-populates one for every existing mixerStore
  // channel, so this is live well before any note actually plays on it).
  getTrackAnalyser(trackId: number): AnalyserNode | null {
    return this.trackChains.get(trackId)?.analyser ?? null;
  }

  // Eagerly constructs the AudioContext without resuming it, so the actual
  // first user-gesture-triggered sound doesn't pay context-construction
  // latency on top of its own sample load — safe to call anytime since
  // construction (unlike resume()) never requires a user gesture.
  warmUp(): void {
    if (!this.ctx) this.ctx = this.buildCtx();
  }

  private async ensureCtx(): Promise<AudioContext> {
    if (!this.ctx) this.ctx = this.buildCtx();
    if (this.ctx.state === 'suspended') await this.ctx.resume();
    return this.ctx;
  }

  // Fire-and-forget warm-up of a channel's sample(s) into the cache ahead of
  // playback — piggybacks entirely on loadSample()/resolveFolder()'s own
  // dedup Maps, so this never duplicates an IPC/decode already in flight or
  // already cached, whether triggered by preload, real playback, or both.
  preloadChannel(ch: ChannelData): void {
    if (ch.samplePath) this.loadSample(ch.samplePath);
    if (ch.sampleFolder) {
      const folderPath = ch.sampleFolder;
      this.resolveFolder(folderPath).then(folder => {
        if (!folder) return;
        // Mirrors pickFromFolder()'s own selection rule: once any pitched
        // entries exist, only those are ever chosen; allPaths is only used
        // as the random fallback when pitched is empty.
        const paths = folder.pitched.length > 0 ? folder.pitched.map(e => e.path) : folder.allPaths;
        for (const p of paths) this.loadSample(p);
      });
    }
  }

  preloadChannels(channels: ChannelData[]): void {
    for (const ch of channels) this.preloadChannel(ch);
  }

  // Same fire-and-forget warm-up as preloadChannel(s), for the two
  // metronome click samples — so toggling the metronome on right before hitting
  // play doesn't pay a cold IPC+decode round trip on the very first click.
  preloadMetronome(): void {
    this.loadSample(METRONOME_ACCENT_PATH);
    this.loadSample(METRONOME_CLICK_PATH);
  }

  async loadSample(relativePath: string): Promise<AudioBuffer | null> {
    if (this.bufferCache.has(relativePath)) return this.bufferCache.get(relativePath)!;
    if (this.loadingPromises.has(relativePath)) return this.loadingPromises.get(relativePath)!;
    const p = (async () => {
      try {
        // Raw binary IPC response — arrives as an ArrayBuffer (see lib.rs).
        const bytes = await invoke<ArrayBuffer>('read_audio_bytes', { relativePath });
        const ctx   = await this.ensureCtx();
        const buf   = await ctx.decodeAudioData(bytes);
        this.bufferCache.set(relativePath, buf);
        return buf;
      } catch {
        return null;
      } finally {
        this.loadingPromises.delete(relativePath);
      }
    })();
    this.loadingPromises.set(relativePath, p);
    return p;
  }

  // One-off audition of a channel's sound at `pitch`, independent of the
  // scheduler/transport — used by the Piano Roll to preview a note on
  // click/place/move. Safe to call at any time; lazily creates the
  // AudioContext exactly like loadSample() does.
  // `props` (the note's own properties) applies its pan/fine pitch/filter/
  // release to the audition; velocity stays the fixed preview velocity.
  async previewNote(ch: ChannelData, pitch: number, props?: NoteProps): Promise<void> {
    const ctx = await this.ensureCtx();
    const playTime = ctx.currentTime + RAMP;
    const sounding = props ? soundingPitch({ pitch, finePitch: props.finePitch }) : pitch;
    const pan = props ? combinePan(ch.pan, props.pan) : ch.pan;
    const opts = props && voiceOpts(ch, props, sounding);
    this.triggerPitched(ch, sounding, ch.volume * this.previewVelocity, pan, ch.mixerTrack, playTime, this.previewDuration, true, opts);
    this.noteSink?.(ch, pitch, this.previewVelocity, playTime, this.previewDuration);
  }

  // ── Live (held) voices — MIDI input / QWERTY keyboard ─────────────────
  // Plays `ch`'s sound at `pitch` until releaseLiveVoice(id): attack → decay
  // → hold at sustain (the same ADSR constants piano-roll notes use, just
  // with an open-ended sustain). Routed through the channel's mixer track,
  // but deliberately ignores Channel Rack mute/solo — live input always
  // auditions the selected channel. Returns an id
  // synchronously so a note-off can never race a still-loading sample.
  startLiveVoice(ch: ChannelData, pitch: number, velocity: number, bend = 0): number {
    const id = this.nextVoiceId++;
    const voice: LiveVoice = { baseRate: 1, bend, released: false };
    this.liveVoices.set(id, voice);
    void (async () => {
      const ctx = await this.ensureCtx();
      const picked = await this.resolvePitched(ch, pitch);
      const buf = picked && await this.loadSample(picked.path);
      if (!picked || !buf || voice.released || this.ctx !== ctx || !this.masterOut) {
        this.liveVoices.delete(id);
        return;
      }
      const time = ctx.currentTime + RAMP;
      const vol = Math.max(0, Math.min(2, ch.volume * velocity));
      const src = ctx.createBufferSource();
      src.buffer = buf;
      voice.baseRate = picked.rate;
      src.playbackRate.value = picked.rate * 2 ** (voice.bend / 12);
      const gain = ctx.createGain();
      const panner = ctx.createStereoPanner();
      panner.pan.value = ch.pan * 2 - 1;
      src.connect(gain).connect(panner).connect(this.resolveDestination(ch.mixerTrack));
      gain.gain.setValueAtTime(0, time);
      gain.gain.linearRampToValueAtTime(vol, time + ATTACK);
      gain.gain.linearRampToValueAtTime(vol * SUSTAIN, time + ATTACK + DECAY);
      src.onended = () => {
        disconnectAll(src, gain, panner); // see playSample
        if (this.liveVoices.get(id) === voice) this.liveVoices.delete(id);
      };
      src.start(time);
      voice.src = src;
      voice.gain = gain;
    })();
    return id;
  }

  releaseLiveVoice(id: number): void {
    const voice = this.liveVoices.get(id);
    if (!voice) return;
    voice.released = true;
    this.liveVoices.delete(id);
    if (!voice.src || !voice.gain || !this.ctx) return;
    const t = this.ctx.currentTime;
    const g = voice.gain.gain;
    g.cancelScheduledValues(t);
    g.setValueAtTime(g.value, t);
    g.linearRampToValueAtTime(0, t + RELEASE);
    voice.src.stop(t + RELEASE + 0.01);
  }

  // Pitch bend: offsets a held voice by `semitones` from its own note pitch
  // (absolute, not cumulative — each call replaces the previous bend).
  setLiveVoiceBend(id: number, semitones: number): void {
    const voice = this.liveVoices.get(id);
    if (!voice) return;
    voice.bend = semitones;
    if (!voice.src || !this.ctx) return;
    const rate = voice.baseRate * 2 ** (semitones / 12);
    voice.src.playbackRate.setTargetAtTime(rate, this.ctx.currentTime, BEND_SMOOTHING);
  }

  releaseAllLiveVoices(): void {
    for (const id of [...this.liveVoices.keys()]) this.releaseLiveVoice(id);
  }

  // Same sample/rate resolution as triggerPitched(), but returned instead
  // of scheduled — live voices need their own source node handle.
  private async resolvePitched(ch: ChannelData, pitch: number): Promise<{ path: string; rate: number } | null> {
    if (ch.sampleFolder) {
      const folder = await this.resolveFolder(ch.sampleFolder);
      return (folder && pickFromFolder(folder, pitch)) || null;
    }
    if (ch.samplePath) return { path: ch.samplePath, rate: 2 ** ((pitch - 60) / 12) };
    return null;
  }

  // ── Preferences hooks ─────────────────────────────────────────────────
  setNoteSink(sink: NoteSink | null): void {
    this.noteSink = sink;
  }

  // Called from stop() whenever a running transport actually stops (also on
  // each restart, which goes through stop() first) — midiOutput.ts uses it
  // to cancel queued MIDI and silence anything still sounding.
  setStopHook(hook: (() => void) | null): void {
    this.stopHook = hook;
  }

  setSchedulerTiming(lookaheadMs: number, tickMs: number): void {
    this.lookahead = Math.max(0.02, lookaheadMs / 1000);
    this.interval = Math.max(5, tickMs);
  }

  setPreview(velocity: number, durationMs: number): void {
    this.previewVelocity = Math.max(0, Math.min(1, velocity));
    this.previewDuration = Math.max(0.02, durationMs / 1000);
  }

  getLatencyInfo(): { baseLatency: number; outputLatency: number; sampleRate: number } | null {
    if (!this.ctx) return null;
    return {
      baseLatency: this.ctx.baseLatency ?? 0,
      outputLatency: (this.ctx as AudioContext & { outputLatency?: number }).outputLatency ?? 0,
      sampleRate: this.ctx.sampleRate,
    };
  }

  get outputLatency(): number {
    return (this.ctx as (AudioContext & { outputLatency?: number }) | null)?.outputLatency ?? 0;
  }

  // Fired after rebuildContext() swaps in a fresh AudioContext, so the
  // mixer bridge (mixerAudio.svelte.ts) can rebuild every track chain and
  // routing edge on the new graph. Returns an unsubscribe function.
  onContextRebuilt(fn: () => void): () => void {
    this.rebuildListeners.add(fn);
    return () => this.rebuildListeners.delete(fn);
  }

  // Applies a new latencyHint/sampleRate — both are construction-time-only
  // AudioContext options, so the whole graph is torn down and rebuilt. The
  // caller is responsible for stopping playback first (playbackStore owns
  // isPlaying). Decoded AudioBuffers aren't tied to a context, so the sample
  // cache survives. No-op if the options didn't change, or (just storing
  // them) if no context exists yet.
  async rebuildContext(opts: AudioContextOptions): Promise<void> {
    const same = opts.latencyHint === this.ctxOptions.latencyHint && opts.sampleRate === this.ctxOptions.sampleRate;
    this.ctxOptions = { ...opts };
    if (same || !this.ctx) return;
    this.stop();
    this.releaseAllLiveVoices();
    const old = this.ctx;
    this.ctx = null;
    this.trackChains.clear();
    this.masterOut = null;
    this.masterVolumeGain = null;
    this.analyser = this.analyserL = this.analyserR = null;
    try { await old.close(); } catch { /* already closed */ }
    this.ctx = this.buildCtx();
    for (const fn of [...this.rebuildListeners]) fn();
  }

  private async resolveFolder(folderPath: string): Promise<MultisampleFolder | null> {
    if (this.folderCache.has(folderPath)) return this.folderCache.get(folderPath)!;
    if (this.folderLoadingPromises.has(folderPath)) return this.folderLoadingPromises.get(folderPath)!;
    const p = (async () => {
      try {
        const names = await invoke<string[]>('list_dir_files', { relativePath: folderPath });
        const resolved = buildMultisample(names, folderPath);
        this.folderCache.set(folderPath, resolved);
        return resolved;
      } catch {
        return null;
      } finally {
        this.folderLoadingPromises.delete(folderPath);
      }
    })();
    this.folderLoadingPromises.set(folderPath, p);
    return p;
  }

  // Schedules one hit for a channel at targetPitch (a note's own pitch, or
  // 60 for a plain step-sequencer trigger) — resolves either a single sample
  // (rate relative to middle C, as always) or a multisample folder (rate
  // relative to whichever recorded sample is nearest targetPitch).
  private triggerPitched(ch: ChannelData, targetPitch: number, vol: number, pan: number, mixerTrack: number, playTime: number, durationSeconds?: number, allowLate = false, voice?: NoteVoiceOpts) {
    if (ch.sampleFolder) {
      const folderPath = ch.sampleFolder;
      this.resolveFolder(folderPath).then(folder => {
        const picked = folder && pickFromFolder(folder, targetPitch);
        if (picked) this.scheduleHit(picked.path, picked.rate, vol, pan, mixerTrack, playTime, durationSeconds, allowLate, voice);
      });
    } else if (ch.samplePath) {
      const rate = 2 ** ((targetPitch - 60) / 12);
      this.scheduleHit(ch.samplePath, rate, vol, pan, mixerTrack, playTime, durationSeconds, allowLate, voice);
    }
  }

  private scheduleHit(path: string, rate: number, vol: number, pan: number, mixerTrack: number, playTime: number, durationSeconds?: number, allowLate = false, voice?: NoteVoiceOpts) {
    this.loadSample(path).then(buf => {
      if (!buf || !this.ctx) return;
      // If the load resolved more than STALE_CUTOFF_SEC after the beat, skip
      // it — playing a stale beat causes a pile-up of out-of-time hits.
      // allowLate (previewNote's one-shot auditions) bypasses this entirely.
      // Within the cutoff, clamp to now so the gain schedule stays valid.
      if (!allowLate && this.ctx.currentTime - playTime > STALE_CUTOFF_SEC) { profiler.count('audio.staleDrop'); return; }
      this.playSample(buf, vol, pan, mixerTrack, Math.max(playTime, this.ctx.currentTime + RAMP), rate, durationSeconds, voice);
    });
  }

  // One metronome click — always straight to Master (center pan, no
  // channel/mixer-track identity of its own), reusing playSample()'s plain
  // one-shot path (durationSeconds left undefined: ring out to the click
  // sample's own short end with just a de-click ramp) exactly like any
  // other sample trigger. Same staleness guard as scheduleHit(), since a
  // click scheduled well behind the transport is just as audibly wrong as a
  // stale beat would be.
  private scheduleMetronomeClick(accent: boolean, playTime: number) {
    const path = accent ? METRONOME_ACCENT_PATH : METRONOME_CLICK_PATH;
    this.loadSample(path).then(buf => {
      if (!buf || !this.ctx) return;
      if (this.ctx.currentTime - playTime > STALE_CUTOFF_SEC) return;
      this.playSample(buf, METRONOME_VOLUME, 0.5, MASTER_TRACK_ID, Math.max(playTime, this.ctx.currentTime + RAMP));
    });
  }

  // Resolves a triggered sample's Web Audio destination from the channel's
  // mixerTrack id: Master itself, the matching mixer track's chain, or —
  // when mixerTrack doesn't match any currently-existing mixer channel —
  // straight to Master, matching this app's implicit pre-mixer behavior.
  // Only called from playSample() once masterVolumeGain is already known to
  // exist (it's built alongside masterOut, which playSample already guards).
  private resolveDestination(mixerTrack: number): AudioNode {
    if (mixerTrack === MASTER_TRACK_ID) return this.masterVolumeGain!;
    return this.trackChains.get(mixerTrack)?.input ?? this.masterVolumeGain!;
  }

  private playSample(buf: AudioBuffer, volume: number, pan: number, mixerTrack: number, time: number, playbackRate = 1, durationSeconds?: number, voice?: NoteVoiceOpts) {
    if (!this.ctx || !this.masterOut) return;
    profiler.count('audio.voiceStart');
    this.activeVoices++;
    const src  = this.ctx.createBufferSource();
    src.buffer = buf;
    src.playbackRate.value = playbackRate;
    const gain = this.ctx.createGain();
    // Channel volume now goes up to 200% (see ChannelRow's Volume dial) —
    // clamped to 2 rather than 1, with the master compressor guarding
    // against clipping when a boosted channel is at or near full velocity.
    const vol  = Math.max(0, Math.min(2, volume));
    const panner = this.ctx.createStereoPanner();
    panner.pan.value = pan * 2 - 1; // 0..1 → -1..1
    let filter: BiquadFilterNode | null = null;
    if (voice && (voice.modX < 1 || voice.modY > 0)) {
      filter = this.ctx.createBiquadFilter();
      filter.type = 'lowpass';
      filter.frequency.value = filterCutoff(voice.modX);
      filter.Q.value = filterQ(voice.modY);
      src.connect(gain).connect(filter).connect(panner).connect(this.resolveDestination(mixerTrack));
    } else {
      src.connect(gain).connect(panner).connect(this.resolveDestination(mixerTrack));
    }
    // Unhook the whole per-voice chain the moment it finishes. Left
    // connected, every finished hit's gain/panner(/filter) stays in the
    // mixer track's graph until JS garbage collection, and WebKit then
    // tears the accumulated batch down under the audio graph lock — a
    // short freeze of both the UI and the audio every few seconds during
    // playback.
    src.onended = () => {
      this.activeVoices--;
      disconnectAll(src, gain, filter, panner);
    };

    if (durationSeconds !== undefined) {
      // Piano-roll note: envelope + explicit stop scaled to the note's own
      // length, so it actually ends when the piano roll says it should
      // instead of ringing out to the sample's full recorded duration.
      // The release fade starts at most 30% of the note before its end (the
      // original fixed RELEASE fits inside the note); a longer "rel" just
      // keeps fading past the note's end.
      const attack  = Math.min(ATTACK, durationSeconds * 0.3);
      const decay   = Math.min(DECAY, Math.max(0, durationSeconds * 0.3 - attack));
      const release = releaseSeconds(voice?.release ?? 0.5);
      const sustainLevel = vol * SUSTAIN;
      const decayEnd     = time + attack + decay;
      const releaseStart = Math.max(decayEnd, time + durationSeconds - Math.min(release, durationSeconds * 0.3));

      gain.gain.setValueAtTime(0, time);
      gain.gain.linearRampToValueAtTime(vol, time + attack);
      gain.gain.linearRampToValueAtTime(sustainLevel, decayEnd);
      gain.gain.setValueAtTime(sustainLevel, releaseStart);
      gain.gain.linearRampToValueAtTime(0, releaseStart + release);

      src.start(time);
      src.stop(releaseStart + release);
      if (voice) this.slides.registerVoice(voice.channelId, voice.color, src, voice.pitch, playbackRate, time, time + durationSeconds);
    } else {
      // Plain step-sequencer one-shot: ring out to the sample's own end,
      // unchanged from before notes existed.
      // Start at full volume immediately — the 0.5ms micro-ramp only guards
      // against DC-offset pops and is inaudible on transient material.
      gain.gain.setValueAtTime(0, time);
      gain.gain.linearRampToValueAtTime(vol, time + RAMP);
      // Fade out the last 0.5ms to prevent an end-of-buffer click. Playback
      // rate stretches/compresses the buffer's audible duration, so the end
      // time must scale with it too.
      const end = time + buf.duration / playbackRate;
      gain.gain.setValueAtTime(vol, Math.max(time + RAMP, end - RAMP));
      gain.gain.linearRampToValueAtTime(0, end);
      src.start(time);
    }
  }

  // One channel's worth of triggering for a single step — pattern mode
  // calls this directly per getChannels() entry; song mode calls it once
  // per active placement's own channel set (getPatternChannels), with
  // `step` already translated to be relative to that placement's own
  // start, and `loopBound` set to the placement's own lengthSteps rather
  // than the pattern-mode activeLength — see tick() below. Extracted
  // unchanged from what used to be tick()'s only per-channel loop body.
  // `loopNotes` (default true, pattern mode's existing behavior) controls
  // whether a note-based channel loops on its own content the same way a
  // step-based channel does; song mode passes false — a placed pattern's
  // step-sequenced channels tile to fill the block, but its piano-roll
  // channels should just play through once, since songTimeline.svelte.ts already
  // sizes the block to the longest channel's own content (see
  // patternContentLength), so a shorter note channel repeating there would
  // read as an unwanted extra repeat rather than the drum-style "same loop
  // again" a step channel wants.
  //
  // `anySoloed` — Channel Rack solo (Ctrl+click a channel's mute dot in
  // ChannelRow): when any channel is soloed, only soloed channels are
  // audible; an explicit mute still always wins regardless (same rule as
  // mixerStore.isAudible, mirrored here as channelStore.isChannelAudible —
  // duplicated inline rather than called from here, since audioEngine.ts
  // deliberately never imports channelStore itself, only live-getter
  // closures like this one).
  private scheduleChannelStep(ch: ChannelData, step: number, playTime: number, loopBound: number, barLength: number, loopNotes = true, anySoloed = false) {
    if (ch.muted || (anySoloed && !ch.soloed)) return;
    // A channel with no sample can still drive MIDI out from its piano-roll
    // notes (e.g. a hardware synth), so only bail here if it has neither.
    const hasSample = !!(ch.samplePath || ch.sampleFolder);
    if (!hasSample && !(ch.midiOutPort && ch.notes.length > 0)) return;
    const pan = ch.pan;

    // Each channel loops on its own content length (see channelLoop.ts)
    // rather than all sharing the overall loop bound — a channel with 16
    // beats of content inside a 32-beat region plays twice instead of
    // once-then-silence. A note-based channel with loopNotes=false instead
    // uses the full loopBound as its own loop length, i.e. it never wraps
    // back on itself within this bound — it just plays through once.
    const useLoop = loopNotes || ch.notes.length === 0;
    const loopLength = useLoop ? channelLoopLength(ch, loopBound, barLength) : loopBound;
    const chStep = step % loopLength;

    // A channel with any piano-roll notes plays purely from note data
    // (pitched, velocity-scaled); otherwise it falls back to the plain
    // boolean step sequencer, unchanged from before notes existed.
    if (ch.notes.length > 0) {
      for (const note of ch.notes) {
        // note.start can be fractional (Shift-free-moved/resized in the
        // Piano Roll) — match on its containing whole step, then offset
        // playTime by the sub-step remainder so the fractional position
        // is still audible at the right moment instead of never matching.
        if (Math.floor(note.start) !== chStep) continue;
        const subStepOffset = (note.start - Math.floor(note.start)) * this.stepDuration;
        const durationSeconds = note.length * this.stepDuration;
        const noteTime = playTime + subStepOffset;
        const pitch = soundingPitch(note);
        if (note.slide) {
          // Slide notes never sound or go out as MIDI — they only bend
          // this channel's same-color notes (see slideVoices.ts).
          this.slides.addSlide(ch.id, note.color, noteTime, durationSeconds, pitch, this.ctx?.currentTime ?? 0);
          continue;
        }
        if (hasSample) {
          this.triggerPitched(ch, pitch, ch.volume * note.velocity, combinePan(pan, note.pan), ch.mixerTrack, noteTime, durationSeconds, false, voiceOpts(ch, note, pitch));
        }
        // Only piano-roll notes go out as MIDI — never step-sequencer hits.
        this.noteSink?.(ch, note.pitch, note.velocity, noteTime, durationSeconds);
      }
    } else if (ch.steps[chStep]) {
      this.triggerPitched(ch, 60, ch.volume, pan, ch.mixerTrack, playTime);
    }
  }

  // Voices started minus ended (profiler gauge).
  activeVoices = 0;

  // How far ahead of the audio clock the scheduler is, in ms (profiler
  // gauge; negative = it fell behind and hits get dropped as stale).
  get schedulerLeadMs(): number {
    return this.ctx ? (this.nextStepTime - this.ctx.currentTime) * 1000 : 0;
  }

  get contextState(): string {
    return this.ctx?.state ?? 'none';
  }

  private tick() {
    if (!this.ctx) return;
    const tickStart = profiler.begin();
    if (this.nextStepTime < this.ctx.currentTime) profiler.count('audio.behind');
    const bpm      = this.getBpm();
    this.stepDuration = 60 / bpm / 4; // sixteenth note — 4 steps per beat
    const barLength = this.getBarLength();
    // Pattern mode: the currently-active pattern's content-driven loop
    // bound. Song mode: the whole arrangement's length — see songTimeline.svelte.ts.
    const loopBound = this.mode === 'song' ? this.getSongLength() : this.getActiveLength();
    const anySoloed = this.getAnySoloed();
    const lookAhead = this.ctx.currentTime + this.lookahead;

    while (this.nextStepTime < lookAhead) {
      const step     = this.scheduledStep;
      const playTime = this.nextStepTime;

      if (this.mode === 'pattern') {
        for (const ch of this.getChannels()) {
          this.scheduleChannelStep(ch, step, playTime, loopBound, barLength, true, anySoloed);
        }
      } else {
        // Song mode: `step` is absolute across the whole arrangement.
        // Every placement whose own [startStep, startStep+lengthSteps)
        // range currently contains it gets scheduled, translated to that
        // placement's own local step — several placements (different
        // tracks) can be simultaneously active.
        // Several placements of the same pattern share one channel view
        // per step (getPatternChannels builds fresh objects for inactive
        // patterns — calling it per placement just feeds the GC).
        const channelsByPattern = new Map<number, ChannelData[]>();
        for (const placement of this.getSongPlacements()) {
          if (step < placement.startStep || step >= placement.startStep + placement.lengthSteps) continue;
          const localStep = step - placement.startStep;
          if (placement.kind === 'pattern') {
            let channels = channelsByPattern.get(placement.patternId!);
            if (!channels) {
              channels = this.getPatternChannels(placement.patternId!);
              channelsByPattern.set(placement.patternId!, channels);
            }
            for (const ch of channels) {
              this.scheduleChannelStep(ch, localStep, playTime, placement.lengthSteps, barLength, false, anySoloed);
            }
          } else if (localStep === 0) {
            // Clip: a one-shot, fired once when the placement's region
            // begins — not step-sequenced, rings out to its own natural
            // length. Center pan, straight to Master (a clip has no
            // channel/mixer-track identity of its own).
            this.scheduleHit(placement.samplePath!, 1, 1, 0.5, MASTER_TRACK_ID, playTime);
          }
        }
      }

      // Metronome — one click per quarter note (4 steps, the fixed
      // step-per-beat convention used throughout this file), accented on
      // beat 1 of every bar. `step` is the absolute, already-wrapped
      // position (0 at the start of the loop, or the start of the song),
      // so `% barLength` reliably lands on downbeats regardless of time
      // signature or which mode is active.
      if (this.getMetronomeOn() && step % 4 === 0) {
        this.scheduleMetronomeClick(step % barLength === 0, playTime);
      }

      if (this.mode === 'song') {
        this.scheduledStep += 1;
        if (this.scheduledStep >= loopBound) {
          // Reached the end of the arrangement — stop scheduling (mirrors
          // pressing Stop) instead of wrapping back to 0. Whatever's
          // already been scheduled for this final step still rings out
          // naturally; only *future* scheduling stops here. timerId must
          // be cleared before onSongEnd() so that playbackStore's
          // isPlaying-watching effect — which calls audioEngine.stop() as
          // part of its "not playing" branch once onSongEnd() flips
          // isPlaying off — sees an already-stopped engine instead of
          // redundantly (but harmlessly) clearing a live timer.
          this.timerId = null;
          this.onSongEnd();
          return;
        }
      } else {
        // Wrapped immediately rather than left to grow unbounded, so
        // scheduledStep always stays inside [0, loopBound) — the same
        // reasoning as loopAnchorTime above.
        this.scheduledStep = (this.scheduledStep + 1) % loopBound;
      }
      this.nextStepTime += 60 / bpm / 4;
    }

    profiler.end('audio.tick', tickStart);
    this.timerId = setTimeout(() => this.tick(), this.interval);
  }

  // Where playback is right now, for the rAF loop driving the visual
  // playhead. Mirrors tick()'s scheduling math but derived from real
  // elapsed time (what's audibly playing) rather than the lookahead-scheduled
  // step. Advances loopAnchorTime forward a full cycle at a time instead of
  // computing (elapsed % activeLength) directly off an ever-growing elapsed
  // value, which would jump whenever activeLength changes mid-loop.
  getPosition(): { step: number; fraction: number } {
    if (!this.ctx) return { step: -1, fraction: 0 };
    let elapsed = this.ctx.currentTime - this.loopAnchorTime;
    if (elapsed < 0) return { step: -1, fraction: 0 };
    let loopBound = this.mode === 'song' ? this.getSongLength() : this.getActiveLength();
    while (elapsed >= loopBound * this.stepDuration) {
      this.loopAnchorTime += loopBound * this.stepDuration;
      elapsed = this.ctx.currentTime - this.loopAnchorTime;
      loopBound = this.mode === 'song' ? this.getSongLength() : this.getActiveLength();
    }
    const raw = elapsed / this.stepDuration;
    return { step: Math.floor(raw), fraction: raw };
  }

  async start(
    getBpm:      () => number,
    getChannels: () => ChannelData[],
    getActiveLength: () => number,
    getBarLength: () => number = () => 16,
    getMetronomeOn: () => boolean = () => false,
    getAnySoloed: () => boolean = () => false,
  ) {
    const ctx = await this.ensureCtx();
    this.mode             = 'pattern';
    this.getBpm          = getBpm;
    this.getChannels     = getChannels;
    this.getBarLength    = getBarLength;
    this.getActiveLength = getActiveLength;
    this.getMetronomeOn  = getMetronomeOn;
    this.getAnySoloed    = getAnySoloed;
    this.scheduledStep   = 0;
    this.stepDuration  = 60 / getBpm() / 4;
    this.nextStepTime  = ctx.currentTime + 0.05;
    this.loopAnchorTime = this.nextStepTime;
    this.tick();
  }

  // Song-mode counterpart to start() — plays the Playlist arrangement
  // (getSongPlacements) once through and stops at its end (getSongLength),
  // rather than looping a single pattern forever. See tick()'s mode
  // branching and the stop-at-end block within it.
  async startSong(
    getBpm: () => number,
    getSongPlacements: () => SongPlacement[],
    getPatternChannels: (patternId: number) => ChannelData[],
    getSongLength: () => number,
    getBarLength: () => number = () => 16,
    getMetronomeOn: () => boolean = () => false,
    onSongEnd: () => void = () => {},
    startStep: number = 0,
    getAnySoloed: () => boolean = () => false,
  ) {
    const ctx = await this.ensureCtx();
    this.mode = 'song';
    this.getBpm             = getBpm;
    this.getSongPlacements  = getSongPlacements;
    this.getPatternChannels = getPatternChannels;
    this.getSongLength      = getSongLength;
    this.getBarLength       = getBarLength;
    this.getMetronomeOn     = getMetronomeOn;
    this.onSongEnd          = onSongEnd;
    this.getAnySoloed       = getAnySoloed;
    // A start marker at/after the arrangement's end would end the song on
    // the very first tick, so fall back to the beginning instead.
    const from = startStep >= 0 && startStep < getSongLength() ? Math.floor(startStep) : 0;
    this.scheduledStep   = from;
    this.stepDuration  = 60 / getBpm() / 4;
    this.nextStepTime  = ctx.currentTime + 0.05;
    // Anchor the visual clock so `from` steps have already "elapsed" at the
    // moment the first scheduled step actually sounds.
    this.loopAnchorTime = this.nextStepTime - from * this.stepDuration;
    this.tick();
  }

  stop() {
    this.slides.clear();
    if (this.timerId !== null) {
      clearTimeout(this.timerId);
      this.timerId = null;
      this.stopHook?.();
    }
  }

  // Live-jump a running Song-mode transport to `step` (Playlist ruler click
  // while playing). Same anchor math as startSong(), but synchronous — only
  // valid once startSong() has resumed, which is always true by the time a
  // user can click. Sources already scheduled inside the lookahead window
  // (~100ms) still sound; only future scheduling moves.
  seekToStep(step: number) {
    if (!this.ctx || this.mode !== 'song') return;
    const length = this.getSongLength();
    const target = step >= 0 && step < length ? Math.floor(step) : 0;
    this.scheduledStep = target;
    this.nextStepTime = this.ctx.currentTime + 0.05;
    this.loopAnchorTime = this.nextStepTime - target * this.stepDuration;
  }

  get currentTime() {
    return this.ctx?.currentTime ?? 0;
  }
}

export const audioEngine = new AudioEngine();
