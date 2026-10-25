import { Channel, invoke } from '@tauri-apps/api/core';
import { listPresets } from '../presetLoader';
import catalogUrl from './catalog.json?url';
import { installedPresetPath, PACKS_ROOT, type Catalog, type CatalogInstrument, type CatalogPack } from './types';

interface InstallProgress { doneFiles: number; totalFiles: number; doneBytes: number; totalBytes: number }

interface Job { pack: CatalogPack; inst: CatalogInstrument }

const jobKey = (pack: CatalogPack, inst: CatalogInstrument) => installedPresetPath(pack, inst);

// Downloadable instrument packs: the catalog (lazy-loaded — ~60 KB gzipped),
// which instruments are installed, and a one-at-a-time install queue.
// Ephemeral UI state; what's installed is just what's on disk.
class PackStore {
  catalog = $state<Catalog | null>(null);
  installed = $state(new Set<string>()); // preset paths of installed pack instruments
  queue = $state<string[]>([]);           // job keys waiting, in order
  current = $state<string | null>(null);  // job key downloading now
  progress = $state<InstallProgress | null>(null);
  errors = $state(new Map<string, string>());
  // Bumped whenever the set of installed presets changes, so the preset
  // browser knows to rescan.
  revision = $state(0);

  #jobs = new Map<string, Job>();
  #loading: Promise<Catalog> | null = null;

  loadCatalog(): Promise<Catalog> {
    // Fetched as a static asset (not imported as a module) so TypeScript
    // never infers a type for the big JSON.
    this.#loading ??= fetch(catalogUrl).then(r => r.json()).then((c: Catalog) => {
      this.catalog = c;
      return c;
    });
    return this.#loading;
  }

  async refreshInstalled(): Promise<void> {
    const paths = await listPresets();
    this.installed = new Set(paths.filter(p => p.startsWith(`${PACKS_ROOT}/`)));
  }

  // "VSCO 2 CE · Strings" for an installed pack preset, null otherwise.
  categoryOf(presetPath: string): string | null {
    if (!this.catalog || !presetPath.startsWith(`${PACKS_ROOT}/`)) return null;
    const [, , packId, ...rest] = presetPath.split('/');
    const pack = this.catalog.packs.find(p => p.id === packId);
    const inst = pack?.instruments.find(i => i.sfz === rest.join('/'));
    return pack && inst ? `${pack.name} · ${inst.category}` : pack?.name ?? null;
  }

  status(pack: CatalogPack, inst: CatalogInstrument): 'installed' | 'downloading' | 'queued' | 'idle' {
    const key = jobKey(pack, inst);
    if (this.current === key) return 'downloading';
    if (this.queue.includes(key)) return 'queued';
    if (this.installed.has(key)) return 'installed';
    return 'idle';
  }

  install(pack: CatalogPack, inst: CatalogInstrument): void {
    const key = jobKey(pack, inst);
    if (this.current === key || this.queue.includes(key)) return;
    this.#jobs.set(key, { pack, inst });
    this.errors.delete(key);
    this.errors = new Map(this.errors);
    this.queue = [...this.queue, key];
    if (!this.current) void this.#runQueue();
  }

  // Cancels the running download and drops everything queued.
  cancelAll(): void {
    this.queue = [];
    if (this.current) void invoke('cancel_pack_install');
  }

  async #runQueue(): Promise<void> {
    while (this.queue.length > 0) {
      const key = this.queue[0];
      this.queue = this.queue.slice(1);
      const job = this.#jobs.get(key)!;
      this.current = key;
      this.progress = null;
      const onProgress = new Channel<InstallProgress>();
      onProgress.onmessage = p => { this.progress = p; };
      const files = job.inst.files.map(([dest, bytes, src]) => ({
        url: job.pack.baseUrl + (src ?? dest).split('/').map(encodeURIComponent).join('/'),
        dest: `${PACKS_ROOT}/${job.pack.id}/${dest}`,
        bytes,
      }));
      // The .sfz is first in the catalog's list — download it last, so an
      // instrument only shows up as installed once all its samples are in.
      files.push(files.shift()!);
      try {
        await invoke('install_pack_files', { files, onProgress });
      } catch (e) {
        if (String(e) !== 'cancelled') this.errors = new Map(this.errors).set(key, String(e));
      }
      this.current = null;
      this.progress = null;
      await this.refreshInstalled();
      this.revision++;
    }
  }

  // Deletes an instrument's .sfz and every sample no other installed
  // instrument of the same pack still uses.
  async remove(pack: CatalogPack, inst: CatalogInstrument): Promise<void> {
    const stillUsed = new Set<string>();
    for (const other of pack.instruments) {
      if (other === inst || !this.installed.has(installedPresetPath(pack, other))) continue;
      for (const [dest] of other.files) stillUsed.add(dest);
    }
    const paths = inst.files.map(([dest]) => dest).filter(d => !stillUsed.has(d)).map(d => `${PACKS_ROOT}/${pack.id}/${d}`);
    // .sfz first, so a half-finished removal never leaves a preset pointing
    // at deleted samples.
    await invoke('delete_pack_files', { paths: [installedPresetPath(pack, inst), ...paths] });
    await this.refreshInstalled();
    this.revision++;
  }
}

export const packStore = new PackStore();
