// The three editing tools shared by the Piano Roll (notes) and the Playlist
// (placed patterns/clips) — same semantics in both:
// draw: place/select/move · delete: click/drag erases · marquee: every drag
// is a selection box. The right button deletes in every tool.
export type EditTool = 'draw' | 'delete' | 'marquee';
