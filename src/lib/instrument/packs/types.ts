// Shape of catalog.json (built by scripts/build-pack-catalog.ts).

// [dest, bytes] or [dest, bytes, src]: dest is pack-relative (where the .sfz
// expects it), src the real repo path when its case differs.
export type CatalogFile = [string, number] | [string, number, string];

export interface CatalogInstrument {
  name: string;
  sfz: string;      // pack-relative .sfz path
  category: string;
  bytes: number;
  files: CatalogFile[];
}

export interface CatalogPack {
  id: string;
  name: string;
  license: string;
  credit: string;
  homepage: string;
  baseUrl: string;
  instruments: CatalogInstrument[];
}

export interface Catalog {
  version: number;
  packs: CatalogPack[];
}

export const PACKS_ROOT = 'instruments/packs';

// Data-relative path of an installed instrument's .sfz (= its preset path).
export const installedPresetPath = (pack: CatalogPack, inst: CatalogInstrument) => `${PACKS_ROOT}/${pack.id}/${inst.sfz}`;

export function formatBytes(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)} GB`;
  if (n >= 1e6) return `${Math.round(n / 1e6)} MB`;
  return `${Math.max(1, Math.round(n / 1e3))} KB`;
}
