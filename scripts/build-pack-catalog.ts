// Builds src/lib/instrument/packs/catalog.json — the list of downloadable
// Bloom instruments (see docs/instrument.md › Packs). For each pack (a
// GitHub repo at a fixed tag) it lists every .sfz, parses it with Bloom's own
// parser and records exactly which files it needs, so the app can install
// one instrument at a time instead of a multi-GB archive.
//
// Run: npm run build:pack-catalog
// (uses the unauthenticated GitHub API: 1 tree request per pack, plus one
// raw download per .sfz)
import { writeFileSync } from 'node:fs';
import { parseSfz } from '../src/lib/instrument/sfz';

interface PackSource {
  id: string;
  name: string;
  repo: string;
  ref: string;
  license: string;
  credit: string;
  homepage: string;
  // Category for an .sfz: VSCO keeps its .sfz files at the root, so its
  // category comes from where the samples live instead.
  category: (sfzPath: string, firstSample: string) => string;
}

const PACKS: PackSource[] = [
  {
    id: 'vsco2',
    name: 'VSCO 2 CE',
    repo: 'sgossner/VSCO-2-CE',
    ref: '1.1.0',
    license: 'CC0-1.0',
    credit: 'Versilian Studios Chamber Orchestra 2 Community Edition',
    homepage: 'https://versilian-studios.com/vsco-community/',
    category: (_sfz, sample) => sample.split('/')[0] || 'Other',
  },
  {
    id: 'vcsl',
    name: 'VCSL',
    repo: 'sgossner/VCSL',
    ref: 'v1.2.2-RC',
    license: 'CC0-1.0',
    credit: 'Versilian Community Sample Library',
    homepage: 'https://github.com/sgossner/VCSL',
    category: (sfz) => sfz.split('/').slice(0, -1).pop() || 'Other',
  },
];

// [dest, bytes] or [dest, bytes, src] — dest is where the .sfz expects the
// file (pack-relative), src the real repo path when its case differs.
type CatalogFile = [string, number] | [string, number, string];
interface CatalogInstrument { name: string; sfz: string; category: string; bytes: number; files: CatalogFile[] }
interface CatalogPack {
  id: string; name: string; license: string; credit: string; homepage: string;
  baseUrl: string; // raw file URL prefix; src paths are appended (URL-encoded per segment)
  instruments: CatalogInstrument[];
}

const opcodeStats = new Map<string, number>();
const raw = (p: PackSource, path: string) =>
  `https://raw.githubusercontent.com/${p.repo}/${p.ref}/${path.split('/').map(encodeURIComponent).join('/')}`;

async function buildPack(p: PackSource): Promise<CatalogPack> {
  const res = await fetch(`https://api.github.com/repos/${p.repo}/git/trees/${p.ref}?recursive=1`);
  if (!res.ok) throw new Error(`${p.repo}: tree ${res.status}`);
  const tree = (await res.json()) as { tree: { path: string; type: string; size?: number }[]; truncated: boolean };
  if (tree.truncated) throw new Error(`${p.repo}: tree truncated`);
  const blobs = new Map(tree.tree.filter(e => e.type === 'blob').map(e => [e.path, e.size ?? 0]));
  const byLower = new Map([...blobs.keys()].map(k => [k.toLowerCase(), k]));
  const sfzPaths = [...blobs.keys()].filter(k => k.toLowerCase().endsWith('.sfz')).sort();

  const instruments: CatalogInstrument[] = [];
  let missing = 0;
  for (const sfz of sfzPaths) {
    const text = await (await fetch(raw(p, sfz))).text();
    for (const m of text.matchAll(/([A-Za-z_]+?)(?:\d+)?=/g)) opcodeStats.set(m[1], (opcodeStats.get(m[1]) ?? 0) + 1);
    const inst = parseSfz(text, sfz);
    const files: CatalogFile[] = [[sfz, blobs.get(sfz)!]];
    const seen = new Set<string>();
    for (const r of inst.regions) {
      if (seen.has(r.sample)) continue;
      seen.add(r.sample);
      // Libraries made on Windows often differ in case from the real file;
      // download the real one but save it where the .sfz expects it.
      const real = blobs.has(r.sample) ? r.sample : byLower.get(r.sample.toLowerCase());
      if (!real) { missing++; continue; }
      files.push(real === r.sample ? [r.sample, blobs.get(real)!] : [r.sample, blobs.get(real)!, real]);
    }
    if (files.length < 2) continue; // nothing playable
    const firstSample = files[1][0];
    instruments.push({
      name: inst.name,
      sfz,
      category: p.category(sfz, firstSample),
      bytes: files.reduce((s, f) => s + f[1], 0),
      files,
    });
    process.stderr.write('.');
  }
  process.stderr.write(`\n${p.id}: ${instruments.length} instruments, ${missing} missing samples\n`);
  return {
    id: p.id, name: p.name, license: p.license, credit: p.credit, homepage: p.homepage,
    baseUrl: `https://raw.githubusercontent.com/${p.repo}/${p.ref}/`,
    instruments,
  };
}

const packs: CatalogPack[] = [];
for (const p of PACKS) packs.push(await buildPack(p));
writeFileSync('src/lib/instrument/packs/catalog.json', JSON.stringify({ version: 1, packs }));
const top = [...opcodeStats.entries()].sort((a, b) => b[1] - a[1]).map(([k, v]) => `${k}:${v}`).join(' ');
process.stderr.write(`opcodes: ${top}\n`);
