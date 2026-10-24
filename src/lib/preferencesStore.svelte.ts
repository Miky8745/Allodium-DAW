import { invoke } from '@tauri-apps/api/core';
import {
  defaultGeneralPrefs, defaultInputPrefs, defaultOutputPrefs,
  type GeneralPrefs, type MidiInputPrefs, type MidiOutputPrefs, type Preferences,
} from './preferencesTypes';

const SAVE_DEBOUNCE_MS = 500;

// Global app preferences (data/preferences.xml) — process-wide, not
// per-project, and deliberately not undo-tracked. Loaded once at app mount;
// startWatching() then debounce-saves on every change via a standalone
// $effect.root (same reason as historyStore: a module singleton has no
// parent effect context).
class PreferencesStore {
  general = $state<GeneralPrefs>(defaultGeneralPrefs());
  inputs = $state<MidiInputPrefs[]>([]);
  outputs = $state<MidiOutputPrefs[]>([]);
  loaded = $state(false);

  #stopWatcher: (() => void) | null = null;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #lastJson = '';

  async load() {
    try {
      const p = await invoke<Preferences | null>('load_preferences');
      if (p) {
        // Spread over defaults so a field added after the file was written
        // still gets a sane value (Rust's #[serde(default)] covers this too,
        // this is just belt-and-braces for the TS side's own types).
        this.general = { ...defaultGeneralPrefs(), ...p.general };
        this.inputs = (p.midiInputs?.input ?? []).map(i => ({ ...defaultInputPrefs(i.name), ...i }));
        this.outputs = (p.midiOutputs?.output ?? []).map(o => ({ ...defaultOutputPrefs(o.name), ...o }));
      }
    } catch (e) {
      console.error('Failed to load preferences', e);
    }
    this.#lastJson = JSON.stringify(this.#serialize());
    this.loaded = true;
  }

  #serialize(): Preferences {
    return {
      general: this.general,
      midiInputs: { input: this.inputs },
      midiOutputs: { output: this.outputs },
    };
  }

  startWatching() {
    if (this.#stopWatcher) return;
    this.#stopWatcher = $effect.root(() => {
      $effect(() => {
        const prefs = this.#serialize();
        const json = JSON.stringify(prefs);
        if (json === this.#lastJson) return;
        if (this.#timer) clearTimeout(this.#timer);
        this.#timer = setTimeout(() => {
          this.#timer = null;
          this.#lastJson = json;
          invoke('save_preferences', { prefs: JSON.parse(json) }).catch(e => console.error('Failed to save preferences', e));
        }, SAVE_DEBOUNCE_MS);
      });
    });
  }

  // Get-or-create by port key, so settings for a device survive it being
  // unplugged and are already there the moment it's plugged back in.
  inputPrefs(name: string): MidiInputPrefs {
    let p = this.inputs.find(i => i.name === name);
    if (!p) {
      this.inputs.push(defaultInputPrefs(name));
      p = this.inputs[this.inputs.length - 1];
    }
    return p;
  }

  outputPrefs(name: string): MidiOutputPrefs {
    let p = this.outputs.find(o => o.name === name);
    if (!p) {
      this.outputs.push(defaultOutputPrefs(name));
      p = this.outputs[this.outputs.length - 1];
    }
    return p;
  }

  // Read-only lookups (no create) for hot paths like incoming messages.
  findInput(name: string): MidiInputPrefs | undefined {
    return this.inputs.find(i => i.name === name);
  }

  findOutput(name: string): MidiOutputPrefs | undefined {
    return this.outputs.find(o => o.name === name);
  }

  get enabledInputNames(): string[] {
    return this.inputs.filter(i => i.enabled).map(i => i.name);
  }

  get enabledOutputNames(): string[] {
    return this.outputs.filter(o => o.enabled).map(o => o.name);
  }
}

export const preferences = new PreferencesStore();
