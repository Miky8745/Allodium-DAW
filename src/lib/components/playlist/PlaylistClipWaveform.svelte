<script lang="ts">
  // Static peak waveform for a placed audio clip — no existing waveform
  // precedent anywhere in this codebase to extend (BlackBox.svelte's canvas
  // is a fixed-resolution real-time oscilloscope, CSS-stretched, not a
  // per-sample static render), built from scratch. Mono/first-channel only.
  import { audioEngine } from '$lib/audioEngine';

  let { samplePath }: { samplePath: string } = $props();

  let containerEl = $state<HTMLDivElement | null>(null);
  let canvasEl = $state<HTMLCanvasElement | null>(null);
  let buf = $state<AudioBuffer | null>(null);

  $effect(() => {
    buf = null;
    audioEngine.loadSample(samplePath).then(b => { buf = b; });
  });

  function draw() {
    if (!containerEl || !canvasEl || !buf) return;
    const dpr = window.devicePixelRatio || 1;
    const cssW = containerEl.clientWidth;
    const cssH = containerEl.clientHeight;
    if (cssW <= 0 || cssH <= 0) return;
    canvasEl.width = Math.max(1, Math.round(cssW * dpr));
    canvasEl.height = Math.max(1, Math.round(cssH * dpr));
    const g = canvasEl.getContext('2d');
    if (!g) return;
    g.scale(dpr, dpr);
    g.clearRect(0, 0, cssW, cssH);

    const data = buf.getChannelData(0);
    const mid = cssH / 2;
    const samplesPerPixel = Math.max(1, Math.floor(data.length / cssW));
    g.strokeStyle = 'rgba(255, 255, 255, 0.75)';
    g.lineWidth = 1;
    g.beginPath();
    for (let x = 0; x < cssW; x++) {
      const start = x * samplesPerPixel;
      const end = Math.min(data.length, start + samplesPerPixel);
      let min = 0, max = 0;
      for (let i = start; i < end; i++) {
        const v = data[i];
        if (v < min) min = v;
        if (v > max) max = v;
      }
      g.moveTo(x + 0.5, mid + min * mid);
      g.lineTo(x + 0.5, mid + max * mid);
    }
    g.stroke();
  }

  $effect(() => {
    // Re-draw whenever the decoded buffer arrives.
    buf;
    draw();
  });

  $effect(() => {
    if (!containerEl) return;
    const ro = new ResizeObserver(() => draw());
    ro.observe(containerEl);
    return () => ro.disconnect();
  });
</script>

<div class="waveform-wrap" bind:this={containerEl}>
  <canvas bind:this={canvasEl}></canvas>
</div>

<style>
  .waveform-wrap {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }

  canvas {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
