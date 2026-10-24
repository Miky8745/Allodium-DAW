import { invoke } from '@tauri-apps/api/core';

// Dev-only profiler (ALLODIUM_PROFILE=1, see src-tauri/src/debug.rs).
// Everything is a no-op until start() — instrumented hot paths call
// begin()/end() (no allocation when disabled) and count().
//
// What it reports, sent to the terminal as `[perf]` lines:
// - every second: frame rate + worst frame, main-thread stalls, DOM
//   mutation count, per-section call count/total/max ms, counters, gauges;
// - immediately, for every main-thread stall ≥ STALL_MS: how long, and
//   which instrumented sections ran during it. A stall with no section
//   (or much longer than its sections) was spent outside instrumented
//   JS — style/layout/paint, GC, or the audio graph.

const STALL_MS = 40;
const PROBE_MS = 10;

interface Stat { count: number; total: number; max: number }
interface Run { name: string; start: number; dur: number }

class Profiler {
  enabled = false;
  #t0 = 0;
  #stats = new Map<string, Stat>();
  #counters = new Map<string, number>();
  #gauges = new Map<string, () => string | number>();
  #recent: Run[] = []; // section runs ≥ 1ms in the last ~2s, for stall attribution
  #pending: string[] = [];
  #frames = 0;
  #worstFrame = 0;
  #stalls = 0;
  #mutations = 0;
  #mutated = new Map<string, number>(); // element description → mutations this second
  #flushTimer: ReturnType<typeof setInterval> | null = null;

  start() {
    if (this.enabled) return;
    this.enabled = true;
    this.#t0 = performance.now();
    this.event('profiler started');

    // Frame pacing.
    let lastFrame = performance.now();
    const frame = (now: number) => {
      this.#frames++;
      this.#worstFrame = Math.max(this.#worstFrame, now - lastFrame);
      lastFrame = now;
      requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);

    // Main-thread stalls: a 10ms interval that fires late was blocked.
    let lastProbe = performance.now();
    setInterval(() => {
      const now = performance.now();
      const gap = now - lastProbe - PROBE_MS;
      lastProbe = now;
      if (gap >= STALL_MS) this.#reportStall(now - gap - PROBE_MS, now, gap);
    }, PROBE_MS);

    new MutationObserver(records => {
      this.#mutations += records.length;
      for (const r of records) {
        const el = (r.target.nodeType === 1 ? r.target : r.target.parentElement) as Element | null;
        const key = el ? describe(el) + (r.type === 'attributes' ? `[${r.attributeName}]` : ` ${r.type}`) : r.type;
        this.#mutated.set(key, (this.#mutated.get(key) ?? 0) + 1);
      }
    })
      .observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });

    this.#flushTimer = setInterval(() => this.#flush(), 1000);
  }

  // `const t = profiler.begin(); …; profiler.end('name', t);`
  begin(): number {
    return this.enabled ? performance.now() : 0;
  }

  end(name: string, t: number) {
    if (!this.enabled) return;
    const now = performance.now();
    const dur = now - t;
    let s = this.#stats.get(name);
    if (!s) this.#stats.set(name, s = { count: 0, total: 0, max: 0 });
    s.count++;
    s.total += dur;
    s.max = Math.max(s.max, dur);
    if (dur >= 1) {
      this.#recent.push({ name, start: t, dur });
      while (this.#recent.length && this.#recent[0].start < now - 2000) this.#recent.shift();
    }
  }

  count(name: string, n = 1) {
    if (!this.enabled) return;
    this.#counters.set(name, (this.#counters.get(name) ?? 0) + n);
  }

  // A value sampled once per flush.
  gauge(name: string, read: () => string | number) {
    this.#gauges.set(name, read);
  }

  event(msg: string) {
    this.#pending.push(`${this.#stamp()} EVENT ${msg}`);
    if (!this.enabled) return;
    this.#send();
  }

  // Flushes and waits for the lines to reach the terminal (before exiting).
  async drain() {
    this.#flush();
    await this.#send();
  }

  #stamp(): string {
    return `t=${((performance.now() - this.#t0) / 1000).toFixed(2)}s`;
  }

  #reportStall(from: number, to: number, gap: number) {
    this.#stalls++;
    const during = this.#recent
      .filter(r => r.start < to && r.start + r.dur > from)
      .map(r => `${r.name} ${r.dur.toFixed(1)}ms`);
    this.#pending.push(`${this.#stamp()} STALL ${gap.toFixed(0)}ms` +
      (during.length ? ` during: ${during.join(', ')}` : ' (no instrumented JS — layout/paint/GC/audio graph?)'));
  }

  #flush() {
    const fmt = (n: number) => n.toFixed(n < 10 ? 2 : 1);
    const sections = [...this.#stats.entries()]
      .sort((a, b) => b[1].total - a[1].total)
      .map(([k, s]) => `${k} n=${s.count} tot=${fmt(s.total)} max=${fmt(s.max)}`);
    const counters = [...this.#counters.entries()].map(([k, v]) => `${k}=${v}`);
    const gauges = [...this.#gauges.entries()].map(([k, read]) => {
      try { return `${k}=${read()}`; } catch { return `${k}=?`; }
    });
    this.#pending.push(
      `${this.#stamp()} fps=${this.#frames} worstFrame=${this.#worstFrame.toFixed(0)}ms stalls=${this.#stalls} mutations=${this.#mutations}` +
      (gauges.length ? ` | ${gauges.join(' ')}` : '') +
      (counters.length ? ` | ${counters.join(' ')}` : '') +
      (sections.length ? `\n    ${sections.join('\n    ')}` : ''),
    );
    const mutated = [...this.#mutated.entries()].sort((a, b) => b[1] - a[1]).slice(0, 6);
    if (mutated.length) this.#pending.push(`    mutated: ${mutated.map(([k, v]) => `${k}×${v}`).join(', ')}`);
    this.#mutated.clear();
    this.#stats.clear();
    this.#counters.clear();
    this.#frames = 0;
    this.#worstFrame = 0;
    this.#stalls = 0;
    this.#mutations = 0;
    this.#send();
  }

  #send(): Promise<void> {
    if (this.#pending.length === 0) return Promise.resolve();
    const lines = this.#pending;
    this.#pending = [];
    return invoke('perf_log', { lines }).then(() => {}, () => {});
  }
}

export const profiler = new Profiler();

// `div.playhead < div.grid < div.playlist` — enough to tell elements apart.
function describe(el: Element): string {
  const parts: string[] = [];
  for (let e: Element | null = el, i = 0; e && i < 3; e = e.parentElement, i++) {
    const cls = [...e.classList].filter(c => !c.startsWith('svelte-')).slice(0, 2).join('.');
    parts.push(e.tagName.toLowerCase() + (cls ? '.' + cls : ''));
  }
  return parts.join(' < ');
}
