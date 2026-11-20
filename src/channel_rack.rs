//! Channel Rack: a vertical list of channels, each with a sample slot, a
//! 16-step sequencer row, pan/volume dials, and mute/solo — wired live to
//! the Stage 2 project model and the Stage 3/4 audio engine. Layout and
//! interaction model ported from Allodium-tauri's own `ChannelRack.svelte`
//! + `ChannelRow.svelte` (see `docs/stage-04-channel-rack.md` for the
//! Stage 4 baseline this builds on).
//!
//! Split by concern rather than kept as one file:
//! - `engine.rs` — thin proxies to `AudioEngine` (playback/transport/
//!   metering).
//! - `patterns.rs` — channel/pattern/placement bookkeeping the Piano
//!   Roll and Playlist read.
//! - `mixer_bus.rs` — mixer-track bookkeeping the Mixer panel reads.
//! - `header.rs` / `row.rs` / `footer.rs` — the three pieces of UI this
//!   view actually draws.
//! - `drag.rs` — the one drag-tracking slot every click-and-drag control
//!   here shares.
//!
//! This file itself keeps only shared state, the handful of methods that
//! don't cleanly belong to one of those (`new`, `push_snapshot`, `save`,
//! `content_for`, `audition`, `add_channel`, `toggle_play`), and the
//! `Render` impl that stitches header/rows/footer together. The
//! `dial`/`scroll_field`/`icon_toggle` controls those UI files build with
//! are themselves in `crate::widgets`, not duplicated per call site.
//!
//! Deliberately **not** ported from the original: the file-picker-based
//! channel creation ("+ Channel" now always creates an empty channel; the
//! only way to give it a sound is dragging a sample onto it from the File
//! Browser), the original's manual `Add steps…`/`Trim steps` menu items
//! (the step row's length instead tracks the selected pattern's own
//! `length_beats` live, and grows itself by one bar once a step in its
//! current last bar is used — see `ChannelRack::ensure_spare_bar`), the
//! mini note-roll row preview for note-based channels, the horizontal
//! scrollbar,
//! and the plugin/MIDI-out sections of the row context menu (no plugin
//! system in this rewrite). Several header toggles (loop start, loop
//! mode, graph editor, piano-roll overview, the options dropdown, the
//! filter dropdown) are carried over as the same **cosmetic, no-op**
//! controls they already are in the original — confirmed against its own
//! source, not a corner cut on this rewrite's part.

mod drag;
mod engine;
mod footer;
mod header;
mod mixer_bus;
mod patterns;
mod row;

use std::collections::HashSet;
use std::sync::Arc;

use gpui::component::input::{InputEvent, InputState};
use gpui::component::scroll::ScrollableElement;
use gpui::{Context, Entity, EventEmitter, IntoElement, Render, Window, prelude::*};

use crate::audio::{self, AudioEngine, SampleBuffer};
use crate::project::{self, Channel, PatternContentEntry, ProjectFile};

use drag::ActiveDrag;

/// Emitted for anything outside the Channel Rack that needs to react to
/// it — today just a dial's hover info, relayed by `AppShell` into the
/// toolbar's own hover-info readout (`ToolbarState::hover`), the same
/// sink every other hoverable control in the app already reports to.
pub enum ChannelRackEvent {
    HoverInfo(&'static str, String),
}

impl EventEmitter<ChannelRackEvent> for ChannelRack {}

const PROJECT_NAME: &str = "default";
const DIAL_SENSITIVITY: f32 = 150.0; // matches Allodium-tauri's Dial.svelte

#[derive(Clone, Copy, PartialEq, Eq)]
enum RackFilter {
    All,
    Unsorted,
}

pub struct ChannelRack {
    engine: Arc<AudioEngine>,
    project: ProjectFile,
    samples: std::collections::HashMap<String, Arc<SampleBuffer>>,
    next_channel_id: u32,
    next_pattern_id: u32,
    next_placement_id: u32,
    next_mixer_track_id: u32,
    drag: Option<ActiveDrag>,
    /// Which channel's notes/content the Piano Roll currently shows — set
    /// by double-clicking a row's sample box or "Open in Piano Roll" in
    /// its context menu, same as the original's `onOpenPianoRoll`.
    selected_channel_id: Option<u32>,
    /// The row `.sel-btn` toggle's own selection set — ctrl/shift-click
    /// range semantics, matching `ChannelRack.svelte`'s `handleSelect`.
    /// Separate from `selected_channel_id`: in the original these are two
    /// unrelated concepts (bulk-select vs. "what the Piano Roll shows").
    multi_selected: HashSet<u32>,
    last_select_idx: Option<usize>,
    renaming_channel_id: Option<u32>,
    rename_input: Entity<InputState>,
    name_col_width: f32,
    // Header toggles below are cosmetic/local-only, same as in the
    // original — see this module's own top doc comment.
    loop_start: bool,
    filter: RackFilter,
    swing: f64,
    loop_mode: bool,
    graph_editor: bool,
    piano_roll_overview: bool,
    /// Bumped on every "Recolor randomly" so repeated clicks keep landing
    /// on different hues instead of recomputing the same one — this
    /// rewrite's stand-in for the original's real RNG (`randomColorfulHex`),
    /// without pulling in a `rand` dependency for one button.
    recolor_seed: u32,
    /// Which channels were soloed right before the last ctrl+click-to-solo
    /// overrode them — ctrl+clicking that same channel's mute button again
    /// un-solos it back to exactly this set instead of just clearing every
    /// solo, so a quick "let me hear just this one" doesn't lose whichever
    /// solo you had going before it. Cleared once restored; overwritten
    /// (not stacked) if you solo a different channel before reverting.
    pre_solo_state: Option<Vec<u32>>,
    /// The step sequencer's last-seen integer step and when it last
    /// changed, so each row can fade its fired step's flash out over
    /// wall-clock time rather than it just sitting lit for as long as the
    /// playhead happens to linger on that step.
    last_step_index: i64,
    last_step_change: std::time::Instant,
}

impl ChannelRack {
    pub fn new(engine: Arc<AudioEngine>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let project = project::load_project(PROJECT_NAME).ok().flatten().unwrap_or_default();

        let mut samples = std::collections::HashMap::new();
        for ch in &project.channels.items {
            if ch.sample_path.is_empty() {
                continue;
            }
            match audio::decode_sample(std::path::Path::new(&ch.sample_path)) {
                Ok(buf) => {
                    samples.insert(ch.sample_path.clone(), buf);
                }
                Err(e) => eprintln!("[channel rack] couldn't load '{}': {e}", ch.sample_path),
            }
        }

        let next_channel_id = project.channels.items.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        let next_pattern_id = project.patterns.items.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let next_placement_id = project.placements.items.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let next_mixer_track_id = project.mixer_tracks.items.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        let selected_channel_id = project.channels.items.first().map(|c| c.id);

        let rename_input = cx.new(|cx| InputState::new(window, cx));
        cx.subscribe(&rename_input, |this: &mut Self, _input, event: &InputEvent, cx| match event {
            InputEvent::PressEnter { .. } | InputEvent::Blur => this.commit_rename(cx),
            _ => {}
        })
        .detach();

        let mut rack = Self {
            engine,
            project,
            samples,
            next_channel_id,
            next_pattern_id,
            next_placement_id,
            next_mixer_track_id,
            drag: None,
            selected_channel_id,
            multi_selected: HashSet::new(),
            last_select_idx: None,
            renaming_channel_id: None,
            rename_input,
            name_col_width: 140.0,
            loop_start: false,
            filter: RackFilter::All,
            swing: 0.0,
            loop_mode: false,
            graph_editor: false,
            piano_roll_overview: false,
            recolor_seed: 0,
            pre_solo_state: None,
            last_step_index: -1,
            last_step_change: std::time::Instant::now(),
        };
        rack.push_snapshot();
        rack
    }

    fn push_snapshot(&mut self) {
        match audio::build_snapshot(&self.project, &self.samples) {
            Ok(snapshot) => self.engine.set_content(snapshot),
            Err(e) => eprintln!("[channel rack] couldn't build snapshot: {e}"),
        }
        self.engine.set_song_content(audio::build_song_snapshot(&self.project, &self.samples));
        let (buses, master_volume) = audio::build_mixer_buses(&self.project);
        self.engine.set_mixer(buses, master_volume);
    }

    fn save(&self) {
        if let Err(e) = project::save_project(PROJECT_NAME, &self.project) {
            eprintln!("[channel rack] save failed: {e}");
        }
    }

    fn content_for(&mut self, channel_id: u32) -> &mut PatternContentEntry {
        let pattern_id = self.project.selected_pattern_id;
        if !self.project.content.items.iter().any(|e| e.pattern_id == pattern_id && e.channel_id == channel_id) {
            self.project.content.items.push(PatternContentEntry {
                pattern_id,
                channel_id,
                steps: Vec::new(),
                notes: vec![],
            });
        }
        self.project
            .content
            .items
            .iter_mut()
            .find(|e| e.pattern_id == pattern_id && e.channel_id == channel_id)
            .unwrap()
    }

    fn audition(&self, channel_id: u32) {
        let Some(ch) = self.project.channels.items.iter().find(|c| c.id == channel_id) else { return };
        let Some(sample) = self.samples.get(&ch.sample_path) else { return };
        self.engine.preview(sample.clone(), ch.volume as f32, ch.pan as f32);
    }

    /// "+ Channel" creates an empty channel outright — no sample picker —
    /// so the only way to give it a sound is dragging a file onto it from
    /// the File Browser (`row.rs`'s `assign_sample` drop target). Shows up
    /// as the row's "Drop sample" placeholder until then.
    fn add_channel(&mut self, cx: &mut Context<Self>) {
        let id = self.next_channel_id;
        self.next_channel_id += 1;
        self.project.channels.items.push(Channel {
            id,
            name: String::new(),
            sample_path: String::new(),
            muted: false,
            soloed: false,
            pan: 0.5,
            volume: 0.8,
            mixer_track: 0,
            color: 0,
        });
        let pattern_id = self.project.selected_pattern_id;
        self.project.content.items.push(PatternContentEntry {
            pattern_id,
            channel_id: id,
            steps: Vec::new(),
            notes: vec![],
        });

        self.selected_channel_id = Some(id);
        self.save();
        cx.notify();
    }

    /// Shared by the `rfd` file picker (`add_channel`) and dropping a
    /// sample from the File Browser straight onto the "+ Channel" button
    /// (`footer.rs`) — the feature-scope brief's own "drag/drop (or at
    /// least pick) a sample file onto a channel," the drag/drop half of
    /// that pair now that a file-explorer panel actually exists to drag
    /// *from* (see `channel_rack.rs`'s own top doc comment, which used to
    /// explain why this wasn't ported yet).
    pub(super) fn add_channel_from_path(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let path_str = path.to_string_lossy().to_string();
        let sample = match audio::decode_sample(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[channel rack] couldn't decode {path_str}: {e}");
                return;
            }
        };
        self.samples.insert(path_str.clone(), sample);

        let id = self.next_channel_id;
        self.next_channel_id += 1;
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Channel".to_string());
        self.project.channels.items.push(Channel {
            id,
            name,
            sample_path: path_str,
            muted: false,
            soloed: false,
            pan: 0.5,
            volume: 0.8,
            mixer_track: 0,
            color: 0,
        });
        let pattern_id = self.project.selected_pattern_id;
        self.project.content.items.push(PatternContentEntry {
            pattern_id,
            channel_id: id,
            steps: Vec::new(),
            notes: vec![],
        });

        self.selected_channel_id = Some(id);
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    /// Dropping a sample from the File Browser onto an *existing* row's
    /// sample box (`row.rs`) replaces that channel's sample in place,
    /// rather than adding a new channel — the drop target decides which
    /// of the two `add_channel_from_path`/`assign_sample` this becomes.
    pub(super) fn assign_sample(&mut self, channel_id: u32, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let path_str = path.to_string_lossy().to_string();
        let sample = match audio::decode_sample(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[channel rack] couldn't decode {path_str}: {e}");
                return;
            }
        };
        self.samples.insert(path_str.clone(), sample);
        let Some(ch) = self.project.channels.items.iter_mut().find(|c| c.id == channel_id) else { return };
        // Dropping a sample onto a channel renames it to match, same as a
        // freshly added channel would be named after its sample.
        ch.name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Channel".to_string());
        ch.sample_path = path_str;
        self.audition(channel_id);
        self.push_snapshot();
        self.save();
        cx.notify();
    }

    /// Decodes and caches a sample by path if it isn't already cached —
    /// shared infrastructure for the Playlist's drag-a-sample-straight-
    /// onto-the-timeline drop, which (unlike `assign_sample`/
    /// `add_channel_from_path`) has no channel to assign the result to.
    pub fn cache_sample(&mut self, path: &std::path::Path) -> Result<Arc<SampleBuffer>, String> {
        let path_str = path.to_string_lossy().to_string();
        if let Some(sample) = self.samples.get(&path_str) {
            return Ok(sample.clone());
        }
        let sample = audio::decode_sample(path)?;
        self.samples.insert(path_str, sample.clone());
        Ok(sample)
    }

    /// A cached sample's decoded data, if it's been loaded — the
    /// Playlist's clip-block waveform reads through this rather than
    /// holding its own copy of `samples`.
    pub fn sample_for(&self, path: &str) -> Option<Arc<SampleBuffer>> {
        self.samples.get(path).cloned()
    }

    pub fn toggle_play(&mut self, cx: &mut Context<Self>) {
        if self.engine.is_playing() {
            self.engine.stop();
        } else {
            self.engine.play(self.project.tempo);
        }
        cx.notify();
    }
}

impl Render for ChannelRack {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui::{MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement, Styled, div};

        window.request_animation_frame();
        cx.notify();

        let playing = self.engine.is_playing();
        let song_mode = self.engine.is_song_mode();
        let current_step = if playing { self.engine.current_step().floor() as i64 } else { -1 };
        if current_step != self.last_step_index {
            self.last_step_index = current_step;
            self.last_step_change = std::time::Instant::now();
        }
        // Each fired step flashes, then fades back to its resting color
        // over this window — independent of how long the playhead
        // actually dwells on that step.
        const STEP_FLASH_FADE_SECS: f32 = 0.18;
        let fired_fade = if current_step >= 0 {
            (1.0 - self.last_step_change.elapsed().as_secs_f32() / STEP_FLASH_FADE_SECS).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let channel_ids: Vec<u32> = self.project.channels.items.iter().map(|c| c.id).collect();
        let pattern_id = self.project.selected_pattern_id;
        let name_col_width = self.name_col_width;
        let rack = cx.entity();

        let header = self.render_header(cx);

        let mut rows = div().size_full().flex().flex_col();
        for (idx, id) in channel_ids.iter().copied().enumerate() {
            rows = rows.child(self.render_row(idx, id, playing, current_step, fired_fade, pattern_id, name_col_width, &rack, cx));
        }

        let footer = self.render_footer(song_mode, cx);

        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family(crate::theme::FONT_UI)
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if this.drag.is_some() {
                    this.update_drag(ev.position, cx);
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| this.end_drag()))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| this.end_drag()))
            .child(header)
            // Horizontal scroll too, not just vertical: patterns can now
            // grow past one bar's worth of steps (`ensure_spare_bar`), so a
            // row's own step grid can be wider than the panel.
            .child(div().flex_1().overflow_hidden().child(rows.overflow_scrollbar()))
            .child(footer)
    }
}
