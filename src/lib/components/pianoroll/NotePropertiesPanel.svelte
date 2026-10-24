<script lang="ts">
  import type { ChannelData } from '$lib/types';
  import { DEFAULT_NOTE_PROPS, FINE_PITCH_RANGE, formatCents, type NoteProps } from '$lib/noteProps';
  import { noteDefaults } from '$lib/noteDefaultsStore.svelte';
  import { formatPan } from '$lib/panFormat';
  import { hoverInfoAction } from '$lib/hoverInfo';
  import NotePropDial from './NotePropDial.svelte';
  import NoteColorPalette from './NoteColorPalette.svelte';

  // Docked on the Piano Roll's left. Edits the selected notes when there
  // are any, otherwise the defaults newly drawn notes get. When the
  // selected notes disagree on a property it's "mixed": the control shows
  // that property's default (dimmed, "Mixed") and nothing is written until
  // the user actually moves it — reading never touches the notes.
  let { channel, selectedNoteIds }: { channel: ChannelData; selectedNoteIds: Set<number> } = $props();

  let selected = $derived(channel.notes.filter(n => selectedNoteIds.has(n.id)));

  function mixed(key: keyof NoteProps): boolean {
    return selected.length > 1 && selected.some(n => n[key] !== selected[0][key]);
  }

  function get<K extends keyof NoteProps>(key: K): NoteProps[K] {
    if (selected.length === 0) return noteDefaults.props[key];
    return mixed(key) ? DEFAULT_NOTE_PROPS[key] : selected[0][key];
  }

  function set<K extends keyof NoteProps>(key: K, value: NoteProps[K]) {
    if (selected.length > 0) {
      for (const n of selected) (n as NoteProps)[key] = value;
    } else {
      noteDefaults.props[key] = value;
    }
  }

  const pct = (v: number) => `${Math.round(v * 100)}%`;
  const d = DEFAULT_NOTE_PROPS;
</script>

<div class="np">
  <div class="np-scope" class:np-scope--sel={selected.length > 0}>
    {selected.length > 0 ? `${selected.length} selected note${selected.length === 1 ? '' : 's'}` : 'New notes'}
  </div>

  <div class="np-dials">
    <NotePropDial label="PAN" name="Note pan" mixed={mixed('pan')} get={() => get('pan')} set={(v) => set('pan', v)} defaultValue={d.pan} format={formatPan} />
    <NotePropDial label="VEL" name="Note velocity" mixed={mixed('velocity')} get={() => get('velocity')} set={(v) => set('velocity', v)} defaultValue={d.velocity} format={pct} />
    <NotePropDial label="REL" name="Note release" mixed={mixed('release')} get={() => get('release')} set={(v) => set('release', v)} defaultValue={d.release} format={pct} />
    <NotePropDial label="MOD X" name="Note mod X (filter cutoff)" mixed={mixed('modX')} get={() => get('modX')} set={(v) => set('modX', v)} defaultValue={d.modX} format={pct} />
    <NotePropDial label="MOD Y" name="Note mod Y (filter resonance)" mixed={mixed('modY')} get={() => get('modY')} set={(v) => set('modY', v)} defaultValue={d.modY} format={pct} />
    <!-- Dial only spans [0, max], so fine pitch (±1200 cents) is offset into 0..2400. -->
    <NotePropDial
      label="PITCH"
      name="Note fine pitch"
      mixed={mixed('finePitch')}
      get={() => get('finePitch') + FINE_PITCH_RANGE}
      set={(v) => set('finePitch', Math.round(v - FINE_PITCH_RANGE))}
      max={FINE_PITCH_RANGE * 2}
      defaultValue={FINE_PITCH_RANGE}
      sensitivity={600}
      format={(v) => formatCents(v - FINE_PITCH_RANGE)}
    />
  </div>

  <button
    class="np-slide"
    class:np-slide--on={get('slide')}
    class:np-slide--mixed={mixed('slide')}
    onclick={() => set('slide', mixed('slide') ? true : !get('slide'))}
    use:hoverInfoAction={{ name: 'Slide (pitch bend) note', value: mixed('slide') ? 'Mixed' : get('slide') ? 'On' : 'Off' }}
    aria-pressed={get('slide')}
  >
    <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
      <path d="M2 11h3c2 0 2-8 4-8h3" />
    </svg>
    Slide
  </button>

  <NoteColorPalette current={mixed('color') ? -1 : get('color')} onPick={(i) => set('color', i)} />
</div>

<style>
  .np {
    width: 138px;
    flex-shrink: 0;
    box-sizing: border-box;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    overflow-y: auto;
    background: #1d1f29;
    border-right: 1px solid var(--explorer-border, #383c4a);
  }

  .np-scope {
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: rgba(255,255,255,0.45);
    text-align: center;
  }

  .np-scope--sel { color: var(--accent); }

  .np-dials {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    row-gap: 8px;
  }

  .np-slide {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 26px;
    font-size: 12px;
    background: rgba(255,255,255,0.04);
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 7px;
    color: rgba(255,255,255,0.55);
    cursor: pointer;
    transition: color 0.1s, background 0.1s, border-color 0.1s;
  }

  .np-slide svg { width: 14px; height: 14px; }

  .np-slide:hover {
    background: rgba(255,255,255,0.07);
    color: rgba(255,255,255,0.8);
  }

  .np-slide--mixed { border-style: dashed; }

  .np-slide--on,
  .np-slide--on:hover {
    color: var(--accent);
    border-color: rgba(230,176,127,0.4);
    background: rgba(230,176,127,0.1);
  }
</style>
