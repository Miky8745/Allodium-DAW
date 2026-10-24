import { untrack } from 'svelte';
import { invoke } from '@tauri-apps/api/core';
import { serializeProject, applyProjectState, saveProject, type ProjectFileData } from './projectSerializer';
import { preferences } from './preferencesStore.svelte';
import { profiler } from './debug/profiler';

// Undo stack size and the idle delay before a commit (+ autosave) both come
// from Preferences > General.
const maxEntries = () => Math.max(1, preferences.general.undoLimit);
const idleMs = () => Math.max(100, preferences.general.autosaveDelayMs);

interface HistoryFile {
  past: ProjectFileData[];
  future: ProjectFileData[];
}

// How long after a commit the history file is written. Coalesces bursts of
// commits into one disk write; stopWatching() flushes anything pending.
const PERSIST_DELAY_MS = 3000;

// Undo/redo for the whole project. Performance notes (this used to stall
// the UI for a noticeable moment ~1s after every edit):
// - Entries are kept as JSON *strings* in plain (non-$state) arrays.
//   Nothing renders the stacks, and as deep $state proxies every entry —
//   up to `undoLimit` full project snapshots — was re-walked through
//   proxies and re-stringified on every commit.
// - The history file is assembled by joining those strings (same on-disk
//   shape as before), and written at most once per PERSIST_DELAY_MS.
// - The watcher effect only serializes (needed to track dependencies);
//   the stringify/compare happens once, when the idle timer commits.
class HistoryStore {
  #past: string[] = [];
  #future: string[] = [];
  // Backed by Preferences (global, persisted) so File > Autosave and the
  // Preferences General tab are the same switch.
  get autosaveEnabled() { return preferences.general.autosaveEnabled; }
  set autosaveEnabled(v: boolean) { preferences.general.autosaveEnabled = v; }
  projectName = $state('');

  #lastJson = '';
  #timer: ReturnType<typeof setTimeout> | null = null;
  #persistTimer: ReturnType<typeof setTimeout> | null = null;
  #stopWatcher: (() => void) | null = null;

  async init(name: string) {
    this.projectName = name;
    this.#past = [];
    this.#future = [];

    const raw = await invoke<string | null>('load_history', { name });
    if (raw) {
      try {
        const parsed = JSON.parse(raw) as HistoryFile;
        this.#past = (parsed.past ?? []).map(e => JSON.stringify(e));
        this.#future = (parsed.future ?? []).map(e => JSON.stringify(e));
      } catch {
        // Corrupt/unreadable history file — start fresh rather than crash.
        this.#past = [];
        this.#future = [];
      }
    }

    this.#lastJson = JSON.stringify(serializeProject());
  }

  // Runs a standalone reactive root (this is a plain module singleton, not a
  // component, so a bare $effect has no parent effect context to attach to).
  // serializeProject() reads every piece of project state, which is what
  // subscribes the effect to all of it; each change just re-arms the idle
  // timer, and the commit (stringify + compare + push + autosave) runs once
  // things go quiet.
  startWatching() {
    if (this.#stopWatcher) return;
    this.#stopWatcher = $effect.root(() => {
      $effect(() => {
        const t = profiler.begin();
        serializeProject();
        profiler.end('history.watch', t);
        if (this.#timer) clearTimeout(this.#timer);
        this.#timer = setTimeout(() => this.#commitIfChanged(), untrack(idleMs));
      });
    });
  }

  stopWatching() {
    this.#stopWatcher?.();
    this.#stopWatcher = null;
    if (this.#timer) clearTimeout(this.#timer);
    this.#timer = null;
    if (this.#persistTimer) {
      clearTimeout(this.#persistTimer);
      this.#persistTimer = null;
      this.#writeHistory();
    }
  }

  #commitIfChanged() {
    this.#timer = null;
    const t = profiler.begin();
    const json = JSON.stringify(serializeProject());
    profiler.end('history.stringify', t);
    if (json === this.#lastJson) return;
    profiler.count('history.commit');
    if (this.#lastJson) {
      this.#past.push(this.#lastJson);
      while (this.#past.length > maxEntries()) this.#past.shift();
    }
    this.#future = [];
    this.#lastJson = json;
    this.#persistHistory();
    if (this.autosaveEnabled) saveProject(this.projectName);
  }

  // Forces any not-yet-committed change into a history entry right now, so
  // Ctrl+Z immediately after an edit has something to undo instead of
  // silently no-oping until the idle timer would otherwise have fired.
  #flushPending() {
    if (!this.#timer) return;
    clearTimeout(this.#timer);
    this.#commitIfChanged();
  }

  // Re-derives the baseline from a fresh live serialization (rather than
  // trusting the popped entry byte-for-byte) so the watcher's next
  // comparison is guaranteed to match — otherwise any reconstruction drift
  // would look like a phantom user edit and could corrupt the other stack.
  #settleOn(json: string) {
    applyProjectState(JSON.parse(json) as ProjectFileData);
    this.#lastJson = JSON.stringify(serializeProject());
  }

  undo() {
    this.#flushPending();
    if (this.#past.length === 0 || !this.#lastJson) return;
    const prev = this.#past.pop()!;
    this.#future.push(this.#lastJson);
    while (this.#future.length > maxEntries()) this.#future.shift();
    this.#settleOn(prev);
    this.#persistHistory();
  }

  redo() {
    this.#flushPending();
    if (this.#future.length === 0 || !this.#lastJson) return;
    const next = this.#future.pop()!;
    this.#past.push(this.#lastJson);
    while (this.#past.length > maxEntries()) this.#past.shift();
    this.#settleOn(next);
    this.#persistHistory();
  }

  #persistHistory() {
    if (this.#persistTimer) return;
    this.#persistTimer = setTimeout(() => {
      this.#persistTimer = null;
      this.#writeHistory();
    }, PERSIST_DELAY_MS);
  }

  // Same {"past":[...],"future":[...]} file as JSON.stringify(HistoryFile)
  // would produce, built from the already-serialized entries.
  #writeHistory() {
    const contents = `{"past":[${this.#past.join(',')}],"future":[${this.#future.join(',')}]}`;
    invoke('save_history', { name: this.projectName, contents });
  }
}

export const historyStore = new HistoryStore();
