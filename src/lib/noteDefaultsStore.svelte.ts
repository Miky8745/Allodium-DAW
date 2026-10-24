import { DEFAULT_NOTE_PROPS, type NoteProps } from './noteProps';

// Properties given to newly drawn Piano Roll notes — what the properties
// panel edits while no notes are selected (the color palette's current
// swatch and the slide toggle included). Ephemeral/session-only, like the
// Piano Roll's tool and zoom.
class NoteDefaultsStore {
  props = $state<NoteProps>({ ...DEFAULT_NOTE_PROPS });
}

export const noteDefaults = new NoteDefaultsStore();
