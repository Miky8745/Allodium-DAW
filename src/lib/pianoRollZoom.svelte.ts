// Session-level UI preference (not saved to the project file / not part of
// undo history), same treatment as channelStore.selectedChannelId.
const MIN_ZOOM = 0.5;
const MAX_ZOOM = 4;
const DEFAULT_ZOOM = 1.75; // more zoomed in than the 1x base (24px->42px steps, 14px->24.5px rows)
const ZOOM_STEP_FACTOR = 1.15; // ~15% per wheel notch / keypress

class PianoRollZoomStore {
  zoom = $state(DEFAULT_ZOOM);

  zoomIn() {
    this.zoom = Math.min(MAX_ZOOM, this.zoom * ZOOM_STEP_FACTOR);
  }

  zoomOut() {
    this.zoom = Math.max(MIN_ZOOM, this.zoom / ZOOM_STEP_FACTOR);
  }
}

export const pianoRollZoom = new PianoRollZoomStore();
