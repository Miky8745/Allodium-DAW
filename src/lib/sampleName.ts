export function formatSampleName(path: string | null): string {
  if (!path) return 'Drop sample';
  const base = path.split('/').pop() ?? path;
  const noExt = base.replace(/\.[^.]+$/, '');
  return noExt
    .replace(/[-_]/g, ' ')
    .replace(/\b\w/g, c => c.toUpperCase());
}

// A channel is either a single sample or a multisample folder; this picks
// whichever is set for display (ChannelRow's drop zone, the Piano Roll's
// header) rather than every caller re-checking both fields. A user-set
// `name` (right-click > Rename in ChannelRow) always wins over the
// sample-derived label.
export function formatChannelLabel(channel: { samplePath: string | null; sampleFolder: string | null; name: string | null }): string {
  return channel.name ?? formatSampleName(channel.sampleFolder ?? channel.samplePath);
}
