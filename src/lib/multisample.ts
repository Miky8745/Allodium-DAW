import { parseNoteName } from './pianoroll/pitch';

export interface MultisampleEntry {
  pitch: number;
  path: string;
}

export interface MultisampleFolder {
  pitched: MultisampleEntry[]; // sorted by pitch ascending
  allPaths: string[];          // every audio file in the folder, for the unpitched fallback
}

// Builds a folder's multisample map from its file listing. Files whose name
// encodes a note (see parseNoteName) become pitched entries used for
// nearest-neighbor lookup; a folder with none of those (e.g. round-robin
// takes like "Guitar Pick (1).wav") falls back to picking any file at
// playback time, same as a single-sample channel.
export function buildMultisample(fileNames: string[], folderPath: string): MultisampleFolder {
  const pitched: MultisampleEntry[] = [];
  const allPaths: string[] = [];
  for (const name of fileNames) {
    const path = `${folderPath}/${name}`;
    allPaths.push(path);
    const pitch = parseNoteName(name);
    if (pitch !== null) pitched.push({ pitch, path });
  }
  pitched.sort((a, b) => a.pitch - b.pitch);
  return { pitched, allPaths };
}

export function nearestMultisampleEntry(entries: MultisampleEntry[], targetPitch: number): MultisampleEntry | null {
  let best: MultisampleEntry | null = null;
  let bestDist = Infinity;
  for (const entry of entries) {
    const dist = Math.abs(entry.pitch - targetPitch);
    if (dist < bestDist) { best = entry; bestDist = dist; }
  }
  return best;
}

// Resolves what to actually play for a note at targetPitch: the nearest
// recorded sample plus the playback-rate offset needed to reach targetPitch
// from it, or (when the folder has no pitch info at all) a random file from
// the folder played at the same middle-C-relative rate a single-sample
// channel would use.
export function pickFromFolder(folder: MultisampleFolder, targetPitch: number): { path: string; rate: number } | null {
  if (folder.pitched.length > 0) {
    const nearest = nearestMultisampleEntry(folder.pitched, targetPitch)!;
    return { path: nearest.path, rate: 2 ** ((targetPitch - nearest.pitch) / 12) };
  }
  if (folder.allPaths.length > 0) {
    const path = folder.allPaths[Math.floor(Math.random() * folder.allPaths.length)];
    return { path, rate: 2 ** ((targetPitch - 60) / 12) };
  }
  return null;
}
