// The Piano Roll's 15-color note palette (Note.color indexes into it).
// Defaults to the Minecraft dye colors minus black (unreadable on the dark
// grid); any swatch can be changed via
// the properties panel's ColorWheel. Saved with the project (.alp
// `noteColors`) and undo-tracked through projectSerializer.ts.
export const DEFAULT_NOTE_COLORS: readonly string[] = [
  '#f9fffe', // white
  '#f9801d', // orange
  '#c74ebd', // magenta
  '#3ab3da', // light blue
  '#fed83d', // yellow
  '#80c71f', // lime
  '#f38baa', // pink
  '#474f52', // gray
  '#9d9d97', // light gray
  '#169c9c', // cyan
  '#8932b8', // purple
  '#3c44aa', // blue
  '#835432', // brown
  '#5e7c16', // green
  '#b02e26', // red
];

export const NOTE_COLOR_COUNT = DEFAULT_NOTE_COLORS.length;
export const LIME_NOTE_COLOR = 5; // default color for new notes

const HEX = /^#[0-9a-f]{6}$/i;

class NoteColorStore {
  palette = $state<string[]>([...DEFAULT_NOTE_COLORS]);

  colorOf(index: number): string {
    return this.palette[index] ?? this.palette[LIME_NOTE_COLOR];
  }

  exportState(): string[] {
    return [...this.palette];
  }

  // Missing/short/invalid entries (projects saved before the palette
  // existed) fall back to the default for that slot.
  importState(colors: string[] | undefined) {
    this.palette = DEFAULT_NOTE_COLORS.map((d, i) => {
      const c = colors?.[i];
      return c && HEX.test(c) ? c.toLowerCase() : d;
    });
  }

  resetToDefault() {
    this.palette = [...DEFAULT_NOTE_COLORS];
  }
}

export const noteColorStore = new NoteColorStore();
