<script lang="ts">
  // Real per-mixer-channel vertical peak meter — same dB-scaling/attack-
  // release/color algorithm as the toolbar's PeakMeter.svelte (via
  // meterMath.ts), just rendered as a vertical fill instead of horizontal.
  // Resolves its AnalyserNode(s) from audioEngine fresh every frame (rather
  // than taking one as a static prop) so it self-heals once
  // mixerAudio.svelte.ts's bridge creates the track's chain — a chain built
  // slightly after this component mounts would otherwise leave a
  // once-computed prop stuck at null forever.
  import { playback } from '$lib/playbackStore.svelte';
  import { audioEngine } from '$lib/audioEngine';
  import { MASTER_TRACK_ID } from '$lib/mixerStore.svelte';
  import { RELEASE, peakOf, ampToFraction, colorForLevel } from '$lib/meterMath';

  let {
    height = 80,
    giant = false,
    trackId,
  }: {
    height?: number;
    giant?: boolean;
    trackId: number;
  } = $props();

  let levelL = $state(0);
  let levelR = $state(0);

  let rafId = 0;
  let dataL: Uint8Array | null = null;
  let dataR: Uint8Array | null = null;
  let ampL = 0;
  let ampR = 0;

  function resolveTaps(): { left: AnalyserNode; right: AnalyserNode } | null {
    if (trackId === MASTER_TRACK_ID) return audioEngine.getStereoAnalysers();
    const a = audioEngine.getTrackAnalyser(trackId);
    return a ? { left: a, right: a } : null;
  }

  function frame() {
    const taps = playback.isPlaying ? resolveTaps() : null;
    if (!taps) {
      levelL = levelR = 0;
      ampL = ampR = 0;
      rafId = 0;
      return;
    }
    if (!dataL || dataL.length !== taps.left.fftSize) dataL = new Uint8Array(taps.left.fftSize);
    if (!dataR || dataR.length !== taps.right.fftSize) dataR = new Uint8Array(taps.right.fftSize);
    const peakL = peakOf(taps.left, dataL);
    const peakR = peakOf(taps.right, dataR);
    ampL = peakL > ampL ? peakL : ampL * RELEASE;
    ampR = peakR > ampR ? peakR : ampR * RELEASE;
    levelL = ampToFraction(ampL);
    levelR = ampToFraction(ampR);
    rafId = requestAnimationFrame(frame);
  }

  // Mirrors PeakMeter.svelte's own rAF convention — only runs while playing.
  $effect(() => {
    if (playback.isPlaying) {
      if (!rafId) rafId = requestAnimationFrame(frame);
    } else {
      if (rafId) cancelAnimationFrame(rafId);
      rafId = 0;
      levelL = levelR = 0;
    }
    return () => { if (rafId) cancelAnimationFrame(rafId); };
  });
</script>

<div class="meter" class:meter--giant={giant} style="height:{height}px" aria-hidden="true">
  <div class="meter-bar">
    <div class="meter-fill" style="height:{levelL * 100}%; background:{colorForLevel(levelL)}"></div>
  </div>
  {#if giant}
    <div class="meter-bar">
      <div class="meter-fill" style="height:{levelR * 100}%; background:{colorForLevel(levelR)}"></div>
    </div>
  {/if}
</div>

<style>
  .meter {
    display: flex;
    gap: 3px;
    align-items: flex-end;
    flex-shrink: 0;
  }

  .meter-bar {
    position: relative;
    width: 8px;
    height: 100%;
    background: #2a2c3a;
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 4px;
    overflow: hidden;
    display: flex;
    align-items: flex-end;
  }

  .meter--giant .meter-bar {
    width: 14px;
  }

  .meter-fill {
    width: 100%;
    border-radius: 3px;
    transition: background 0.1s linear;
  }
</style>
