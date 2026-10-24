<script lang="ts">
  // Self-contained HSV color picker: a hue/saturation wheel (canvas, drawn
  // at full value so it reads as a true color wheel) plus a horizontal
  // value/brightness bar underneath. No color-picker library exists
  // anywhere in this app (checked package.json) — built from scratch, same
  // as every other control (Dial/Slider/ScrollField), and rendered via
  // canvas at devicePixelRatio resolution redrawn on a ResizeObserver, the
  // same responsive-canvas pattern as PlaylistClipWaveform.svelte.
  import { hexToRgb, rgbToHex, hsvToRgb, rgbToHsv } from '$lib/colorUtils';

  let { value = $bindable('#e6b07f') }: { value?: string } = $props();

  // Seeded once from the initial value on mount — this widget is opened
  // fresh each time the context menu's Recolor view is shown, so there's no
  // need to reactively re-sync from external value changes (and doing so
  // would fight the drag's own writes to `value`).
  const [initH, initS, initV] = rgbToHsv(...hexToRgb(value));
  let hue = $state(initH); // 0..360
  let sat = $state(initS); // 0..1
  let val = $state(initV); // 0..1

  function commit() {
    const [r, g, b] = hsvToRgb(hue, sat, val);
    value = rgbToHex(r, g, b);
  }

  // --- Hue/saturation wheel ---------------------------------------------

  let wheelWrapEl = $state<HTMLDivElement | null>(null);
  let wheelEl = $state<HTMLCanvasElement | null>(null);
  let wheelCssSize = $state(0);
  let baseImage: ImageData | null = null;
  let draggingWheel = $state(false);

  function drawWheelBase() {
    if (!wheelEl || wheelCssSize <= 0) return;
    const dpr = window.devicePixelRatio || 1;
    const px = Math.max(1, Math.round(wheelCssSize * dpr));
    wheelEl.width = px;
    wheelEl.height = px;
    const g = wheelEl.getContext('2d');
    if (!g) return;
    const cx = px / 2, cy = px / 2, r = px / 2;
    const img = g.createImageData(px, px);
    for (let y = 0; y < px; y++) {
      for (let x = 0; x < px; x++) {
        const dx = x - cx, dy = y - cy;
        const dist = Math.sqrt(dx * dx + dy * dy);
        const idx = (y * px + x) * 4;
        if (dist > r) { img.data[idx + 3] = 0; continue; }
        let ang = (Math.atan2(dy, dx) * 180) / Math.PI;
        ang = (ang + 360) % 360;
        const s = Math.min(1, dist / r);
        const [rr, gg, bb] = hsvToRgb(ang, s, 1);
        img.data[idx] = rr; img.data[idx + 1] = gg; img.data[idx + 2] = bb; img.data[idx + 3] = 255;
      }
    }
    baseImage = img;
    redrawWheel();
  }

  function redrawWheel() {
    if (!wheelEl || !baseImage) return;
    const g = wheelEl.getContext('2d');
    if (!g) return;
    g.putImageData(baseImage, 0, 0);

    const px = wheelEl.width;
    const cx = px / 2, cy = px / 2, r = px / 2;
    const rad = (hue * Math.PI) / 180;
    const dist = sat * r;
    const ix = cx + Math.cos(rad) * dist;
    const iy = cy + Math.sin(rad) * dist;

    g.beginPath();
    g.arc(ix, iy, 7, 0, Math.PI * 2);
    g.strokeStyle = 'rgba(0,0,0,0.6)';
    g.lineWidth = 3;
    g.stroke();
    g.beginPath();
    g.arc(ix, iy, 7, 0, Math.PI * 2);
    g.strokeStyle = '#fff';
    g.lineWidth = 1.5;
    g.stroke();
  }

  $effect(() => {
    // Re-render the indicator whenever hue/sat change (drag or programmatic).
    hue; sat;
    redrawWheel();
  });

  $effect(() => {
    if (!wheelWrapEl) return;
    const ro = new ResizeObserver(entries => {
      const w = entries[0]?.contentRect.width ?? 0;
      if (w > 0 && Math.abs(w - wheelCssSize) > 0.5) {
        wheelCssSize = w;
        drawWheelBase();
      }
    });
    ro.observe(wheelWrapEl);
    return () => ro.disconnect();
  });

  function updateFromWheelPointer(clientX: number, clientY: number) {
    if (!wheelEl) return;
    const rect = wheelEl.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    const r = rect.width / 2;
    let dx = clientX - cx;
    let dy = clientY - cy;
    let dist = Math.sqrt(dx * dx + dy * dy);
    // Clamp to the wheel's edge instead of ignoring out-of-bounds pointer
    // positions — dragging past the rim should still resolve to a valid
    // color at that hue, not freeze or do nothing (this is the main thing
    // "responsive" hinges on for a circular control).
    if (dist > r && r > 0) {
      const scale = r / dist;
      dx *= scale; dy *= scale;
      dist = r;
    }
    let ang = (Math.atan2(dy, dx) * 180) / Math.PI;
    ang = (ang + 360) % 360;
    hue = ang;
    sat = r > 0 ? dist / r : 0;
    commit();
  }

  // Plain mouse events (not pointer events), matching Dial.svelte/
  // Slider.svelte's drag convention throughout this app. This also matters
  // functionally here: the context menu's backdrop closes on `mousedown`,
  // and a `pointerdown` listener's stopPropagation() would NOT stop the
  // separate compatibility `mousedown` event the browser fires right after
  // it (they're independently dispatched) — using `mousedown` here too is
  // what actually keeps starting a drag on the wheel from also closing the
  // menu out from under it.
  function onWheelMousedown(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    draggingWheel = true;
    updateFromWheelPointer(e.clientX, e.clientY);
  }

  // --- Value (brightness) bar --------------------------------------------

  let valBarEl = $state<HTMLDivElement | null>(null);
  let draggingVal = $state(false);

  function updateFromValPointer(clientX: number) {
    if (!valBarEl) return;
    const rect = valBarEl.getBoundingClientRect();
    const frac = rect.width > 0 ? (clientX - rect.left) / rect.width : 0;
    val = Math.max(0, Math.min(1, frac));
    commit();
  }

  function onValMousedown(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    draggingVal = true;
    updateFromValPointer(e.clientX);
  }

  function onWindowMousemove(e: MouseEvent) {
    if (draggingWheel) updateFromWheelPointer(e.clientX, e.clientY);
    else if (draggingVal) updateFromValPointer(e.clientX);
  }

  function onWindowMouseup() {
    draggingWheel = false;
    draggingVal = false;
  }

  // Full-value color at the current hue/sat — the value bar's right end
  // and the live swatch preview both read off this.
  let fullValueHex = $derived.by(() => {
    const [r, g, b] = hsvToRgb(hue, sat, 1);
    return rgbToHex(r, g, b);
  });
</script>

<svelte:window onmousemove={onWindowMousemove} onmouseup={onWindowMouseup} />

<div class="color-wheel">
  <div class="wheel-wrap" bind:this={wheelWrapEl}>
    <canvas
      bind:this={wheelEl}
      class="wheel-canvas"
      onmousedown={onWheelMousedown}
      role="slider"
      aria-label="Hue and saturation"
      aria-valuenow={Math.round(hue)}
      aria-valuemin={0}
      aria-valuemax={360}
      tabindex="0"
    ></canvas>
  </div>

  <div class="val-row">
    <div
      class="val-bar"
      bind:this={valBarEl}
      onmousedown={onValMousedown}
      style="background: linear-gradient(to right, #000, {fullValueHex});"
      role="slider"
      aria-label="Brightness"
      aria-valuenow={Math.round(val * 100)}
      aria-valuemin={0}
      aria-valuemax={100}
      tabindex="0"
    >
      <div class="val-thumb" style="left:{val * 100}%;"></div>
    </div>
    <div class="swatch" style="background:{value};"></div>
  </div>
</div>

<style>
  .color-wheel {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 6px 8px 8px;
    width: 100%;
    box-sizing: border-box;
  }

  .wheel-wrap {
    width: 100%;
    aspect-ratio: 1;
  }

  .wheel-canvas {
    width: 100%;
    height: 100%;
    display: block;
    cursor: crosshair;
    touch-action: none;
    border-radius: 50%;
  }

  .val-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .val-bar {
    position: relative;
    flex: 1;
    height: 14px;
    border-radius: 7px;
    border: 1px solid rgba(255,255,255,0.12);
    cursor: pointer;
    touch-action: none;
  }

  .val-thumb {
    position: absolute;
    top: 50%;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #fff;
    border: 1.5px solid rgba(0,0,0,0.6);
    box-shadow: 0 1px 3px rgba(0,0,0,0.6);
    transform: translate(-50%, -50%);
    pointer-events: none;
  }

  .swatch {
    width: 20px;
    height: 20px;
    border-radius: 5px;
    border: 1px solid rgba(255,255,255,0.15);
    flex-shrink: 0;
  }
</style>
