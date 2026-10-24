<script lang="ts">
  // Toolbar's stereo peak meter — two horizontal bars driven by the
  // left/right analyser taps off audioEngine's master bus. Scaled in dBFS
  // (not raw linear amplitude) so "max" lines up with actual 0dBFS clipping
  // and normal listening levels don't all bunch up near the left edge — see
  // "Toolbar audio monitor & peak meter" in CLAUDE.md.
  import { audioEngine } from '$lib/audioEngine';
  import { playback } from '$lib/playbackStore.svelte';
  import { RELEASE, peakOf, ampToFraction, colorForLevel } from '$lib/meterMath';
  import { hoverInfoAction } from '$lib/hoverInfo';

  // How long a peak stays marked by the hold line before it can be
  // overtaken by a lower, more recent peak.
  const HOLD_WINDOW_MS = 5000;

  let levelL = $state(0); // 0..1, already dB-scaled
  let levelR = $state(0);
  let holdL  = $state(0); // 0..1, max of levelL over the trailing HOLD_WINDOW_MS
  let holdR  = $state(0);

  let rafId = 0;
  let dataL: Uint8Array | null = null;
  let dataR: Uint8Array | null = null;
  let ampL = 0; // running linear-amplitude peak (pre-dB), attack/release smoothed
  let ampR = 0;
  let historyL: { t: number; level: number }[] = [];
  let historyR: { t: number; level: number }[] = [];

  function pushHold(history: { t: number; level: number }[], level: number, now: number): number {
    history.push({ t: now, level });
    while (history.length && now - history[0].t > HOLD_WINDOW_MS) history.shift();
    let max = 0;
    for (const s of history) if (s.level > max) max = s.level;
    return max;
  }

  function frame() {
    const stereo = audioEngine.getStereoAnalysers();
    if (!playback.isPlaying || !stereo) {
      levelL = levelR = 0;
      holdL = holdR = 0;
      ampL = ampR = 0;
      historyL = [];
      historyR = [];
      rafId = 0;
      return;
    }
    if (!dataL || dataL.length !== stereo.left.fftSize)  dataL = new Uint8Array(stereo.left.fftSize);
    if (!dataR || dataR.length !== stereo.right.fftSize) dataR = new Uint8Array(stereo.right.fftSize);
    const peakL = peakOf(stereo.left, dataL);
    const peakR = peakOf(stereo.right, dataR);
    ampL = peakL > ampL ? peakL : ampL * RELEASE;
    ampR = peakR > ampR ? peakR : ampR * RELEASE;
    levelL = ampToFraction(ampL);
    levelR = ampToFraction(ampR);
    const now = performance.now();
    holdL = pushHold(historyL, levelL, now);
    holdR = pushHold(historyR, levelR, now);
    rafId = requestAnimationFrame(frame);
  }

  // Mirrors playbackStore's own rAF convention — only runs while playing.
  $effect(() => {
    if (playback.isPlaying) {
      if (!rafId) rafId = requestAnimationFrame(frame);
    } else {
      if (rafId) cancelAnimationFrame(rafId);
      rafId = 0;
      levelL = levelR = 0;
      holdL = holdR = 0;
    }
    return () => { if (rafId) cancelAnimationFrame(rafId); };
  });

</script>

<div class="tb-peak" use:hoverInfoAction={{ name: 'Peak meter' }} aria-label="Stereo peak meter">
  <div class="tb-peak-bar">
    <div class="tb-peak-fill" style="width: {levelL * 100}%; background: {colorForLevel(levelL)}"></div>
    <div class="tb-peak-hold" style="left: {holdL * 100}%"></div>
  </div>
  <div class="tb-peak-bar">
    <div class="tb-peak-fill" style="width: {levelR * 100}%; background: {colorForLevel(levelR)}"></div>
    <div class="tb-peak-hold" style="left: {holdR * 100}%"></div>
  </div>
</div>

<style>
  .tb-peak {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 0 10px;
    justify-content: center;
    flex-shrink: 0;
  }
  .tb-peak-bar {
    position: relative;
    width: 72px;
    height: 9px;
    background: #2a2c3a;
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 4px;
    overflow: hidden;
  }
  .tb-peak-fill {
    height: 100%;
    border-radius: 3px;
    transition: background 0.1s linear;
  }
  .tb-peak-hold {
    position: absolute;
    top: 0;
    width: 2px;
    height: 100%;
    background: rgba(255, 255, 255, 0.75);
    transform: translateX(-1px);
  }
</style>
