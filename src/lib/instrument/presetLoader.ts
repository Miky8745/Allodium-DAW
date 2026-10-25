import { invoke } from '@tauri-apps/api/core';
import { parseSfz, type SfzInstrument } from './sfz';

// Parsed-preset cache shared by the audio engine and the instrument window.
// Same dedup shape as audioEngine's bufferCache/loadingPromises.
const cache = new Map<string, SfzInstrument>();
const pending = new Map<string, Promise<SfzInstrument | null>>();

export function getCachedPreset(path: string): SfzInstrument | null {
  return cache.get(path) ?? null;
}

export function loadPreset(path: string): Promise<SfzInstrument | null> {
  const hit = cache.get(path);
  if (hit) return Promise.resolve(hit);
  const inFlight = pending.get(path);
  if (inFlight) return inFlight;
  const p = (async () => {
    try {
      const text = await invoke<string>('read_text_file', { relativePath: path });
      const parsed = parseSfz(text, path);
      cache.set(path, parsed);
      return parsed;
    } catch (e) {
      console.warn(`[instrument] failed to load preset ${path}:`, e);
      return null;
    } finally {
      pending.delete(path);
    }
  })();
  pending.set(path, p);
  return p;
}

export function listPresets(): Promise<string[]> {
  return invoke<string[]>('list_presets');
}

// Drops a preset from the cache so the next load re-reads it from disk
// (the preset browser's refresh — handy while hand-editing an .sfz).
export function forgetPresets(): void {
  cache.clear();
}
