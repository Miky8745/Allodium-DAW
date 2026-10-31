// Forces one cursor app-wide for the duration of a drag, so it stays put
// even when the pointer leaves the element that started the drag (drags
// here are tracked with window/document listeners). Backed by the
// `html.drag-cursor` rule in app.html.
//
// Cursor conventions: `move` = drag in every direction (notes, placements,
// windows, files), `ew-resize` = horizontal-only move (overview windows),
// `col-resize` = extending/resizing an edge, `cell`/`not-allowed` = Piano
// Roll draw/delete tools.

export function beginDragCursor(cursor: string) {
  const root = document.documentElement;
  root.style.setProperty('--drag-cursor', cursor);
  root.classList.add('drag-cursor');
}

export function endDragCursor() {
  document.documentElement.classList.remove('drag-cursor');
}
