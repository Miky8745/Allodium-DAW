//! Core data model + XML persistence, ported from Allodium-tauri's
//! `project.rs`. See `docs/stage-02-data-model.md` for the design choices
//! (which parts were kept, which were trimmed for the MVP scope).
//!
//! Conventions carried over deliberately:
//! - String fields use `""` as "none" rather than `Option<String>` —
//!   quick-xml round-trips `Option::None` as an empty tag that
//!   deserializes back as `Some("")`, not `None`, so a plain sentinel
//!   value sidesteps the mismatch instead of fighting it.
//! - Numeric "none" fields use a sentinel too (`0` = no routing target),
//!   since an empty tag's content isn't even valid text for a numeric
//!   `Option` to parse.
//! - Every field added after the first cut gets `#[serde(default)]` so
//!   older project files keep loading.
//! - A bare `Vec<T>` field repeats its own tag name per element in
//!   quick-xml rather than emitting one wrapper tag, so lists get a
//!   small wrapper struct with a renamed inner field instead.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sample_path: String,
    pub muted: bool,
    #[serde(default)]
    pub soloed: bool,
    pub pan: f64,
    pub volume: f64,
    /// `0` = routed straight to Master; otherwise a `MixerTrack` id.
    pub mixer_track: u32,
    /// Packed `0xRRGGBB` tinting the Channel Rack's sample-name box —
    /// ports Allodium-tauri's own `channel.color` (its `ChannelRow.svelte`
    /// right-click "Recolor"/"Recolor randomly"). `0` = no tint, the same
    /// numeric-sentinel convention `mixer_track` above uses — not
    /// `Option<u32>`, per this file's own top-of-file note on why numeric
    /// fields here use a sentinel rather than `Option`.
    #[serde(default)]
    pub color: u32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: u32,
    pub pitch: i32,
    pub start: f64,
    pub length: f64,
    pub velocity: f64,
}

/// A channel's note content for one specific pattern. Channels are shared
/// across all patterns; their actual note data is stashed per pattern here
/// instead, mirroring Allodium-tauri's "pattern-scoped content" model (see
/// `docs/audio-playback.md` there).
///
/// `steps` only exists to deserialize project files saved before the step
/// sequencer and Piano Roll were unified onto one `notes` backend — see
/// `migrate_steps_to_notes` below, run once on every load. Nothing in this
/// app writes a non-empty `steps` anymore; a step is a `Note { pitch:
/// STEP_PITCH, length: 1.0, .. }` like any other.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PatternContentEntry {
    pub pattern_id: u32,
    pub channel_id: u32,
    #[serde(default)]
    pub steps: Vec<bool>,
    #[serde(default)]
    pub notes: Vec<Note>,
}

/// The step sequencer's fixed pitch — ported from the original's "step
/// channels play unshifted," i.e. the piano roll's own "60 = unity
/// playback rate" reference pitch (`audio::pitch`). Public so
/// `channel_rack::row` (which owns step toggling) and `audio::mod` (which
/// no longer needs it directly now that `steps` is dead weight, but keeps
/// it available for anything that wants to recognize a step-shaped note)
/// can both refer to the same constant instead of repeating the literal.
pub const STEP_PITCH: i32 = 60;

/// One-time upgrade for project files saved before steps/notes were
/// unified: turns each `true` entry in `steps` into an equivalent
/// `Note { pitch: STEP_PITCH, length: 1.0, velocity: 1.0, .. }` and clears
/// `steps`, so every other part of the app can assume `notes` is the only
/// source of truth. Note ids only need to be unique within their own
/// entry's `notes` (nothing compares ids across entries), so a simple
/// per-entry counter starting past whatever's already there is enough —
/// same scheme `piano_roll.rs`'s own `next_note_id` uses.
fn migrate_steps_to_notes(project: &mut ProjectFile) {
    for entry in &mut project.content.items {
        if entry.steps.is_empty() {
            continue;
        }
        let mut next_id = entry.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        for (i, &on) in entry.steps.iter().enumerate() {
            if on {
                entry.notes.push(Note { id: next_id, pitch: STEP_PITCH, start: i as f64, length: 1.0, velocity: 1.0 });
                next_id += 1;
            }
        }
        entry.steps.clear();
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Pattern {
    pub id: u32,
    #[serde(default)]
    pub name: String,
    pub length_beats: f64,
}

/// A block placed on a Playlist track — either a pattern (the common
/// case, length looked up live from the referenced `Pattern.length_beats`)
/// or a one-shot sample "clip" dragged straight in from the File Browser.
/// Distinguished by the same sentinel-over-`kind`-field convention this
/// file already uses elsewhere: `sample_path` empty means "pattern
/// placement, `pattern_id` is the real id"; non-empty means "clip,
/// `pattern_id` is `0`" (pattern ids start at 1, so `0` never collides
/// with a real one). A clip stores its own `length_beats` (computed once
/// from the sample's real duration when dropped) since there's no pattern
/// to look it up from; `0.0` there means "pattern placement, ignore this
/// field."
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub id: u32,
    pub pattern_id: u32,
    pub track_id: u32,
    pub start_beat: f64,
    #[serde(default)]
    pub sample_path: String,
    #[serde(default)]
    pub length_beats: f64,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MixerTrack {
    pub id: u32,
    pub volume: f64,
    pub pan: f64,
    pub muted: bool,
    #[serde(default)]
    pub soloed: bool,
    /// `0` = routed straight to Master; otherwise another `MixerTrack` id.
    #[serde(default)]
    pub routes_to: u32,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ChannelsXml {
    #[serde(rename = "channel", default)]
    pub items: Vec<Channel>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ContentXml {
    #[serde(rename = "entry", default)]
    pub items: Vec<PatternContentEntry>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PatternsXml {
    #[serde(rename = "pattern", default)]
    pub items: Vec<Pattern>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PlacementsXml {
    #[serde(rename = "placement", default)]
    pub items: Vec<Placement>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct MixerTracksXml {
    #[serde(rename = "track", default)]
    pub items: Vec<MixerTrack>,
}

fn default_step_count() -> u32 {
    16
}

fn default_master_volume() -> f64 {
    1.0
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub beats_per_bar: u32,
    #[serde(default = "default_step_count")]
    pub step_count: u32,
    pub tempo: f64,
    pub selected_pattern_id: u32,
    #[serde(default)]
    pub channels: ChannelsXml,
    #[serde(default)]
    pub content: ContentXml,
    #[serde(default)]
    pub patterns: PatternsXml,
    #[serde(default)]
    pub placements: PlacementsXml,
    #[serde(default)]
    pub mixer_tracks: MixerTracksXml,
    /// Master strip's own volume fader. Per `docs/mixer.md`'s documented
    /// behavior, Master has no pan/mute/solo of its own (never mutable or
    /// soloable), so this is its only control — no separate `MasterTrack`
    /// struct needed for one field.
    #[serde(default = "default_master_volume")]
    pub master_volume: f64,
}

impl Default for ProjectFile {
    fn default() -> Self {
        Self {
            beats_per_bar: 4,
            step_count: default_step_count(),
            tempo: 120.0,
            selected_pattern_id: 1,
            channels: ChannelsXml::default(),
            content: ContentXml::default(),
            patterns: PatternsXml {
                items: vec![Pattern { id: 1, name: "Pattern 1".to_string(), length_beats: 4.0 }],
            },
            placements: PlacementsXml::default(),
            mixer_tracks: MixerTracksXml::default(),
            master_volume: default_master_volume(),
        }
    }
}

const PROJECT_EXT: &str = "alp";

fn validate_project_name(name: &str) -> Result<&str, String> {
    let clean = name.trim();
    if clean.is_empty() {
        return Err("Project name cannot be empty".to_string());
    }
    if clean.contains('/') || clean.contains('\\') || clean.contains("..") {
        return Err("Project name contains invalid characters".to_string());
    }
    Ok(clean)
}

/// `~/.local/share/allodium/projects` on Linux. Native app, no IPC
/// boundary to cross, so this is a plain function rather than a
/// `#[tauri::command]`.
pub fn projects_root() -> Result<std::path::PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    Ok(std::path::PathBuf::from(home).join(".local/share/allodium/projects"))
}

pub fn save_project(name: &str, project: &ProjectFile) -> Result<(), String> {
    let clean = validate_project_name(name)?;
    let dir = projects_root()?.join(clean);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{clean}.{PROJECT_EXT}"));
    let mut body = String::new();
    let mut ser = quick_xml::se::Serializer::new(&mut body);
    ser.indent(' ', 2);
    project.serialize(ser).map_err(|e| e.to_string())?;
    let xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{body}\n");
    std::fs::write(&path, xml).map_err(|e| e.to_string())
}

pub fn load_project(name: &str) -> Result<Option<ProjectFile>, String> {
    let clean = validate_project_name(name)?;
    let path = projects_root()?.join(clean).join(format!("{clean}.{PROJECT_EXT}"));
    if !path.exists() {
        return Ok(None);
    }
    let xml = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut project: ProjectFile = quick_xml::de::from_str(&xml).map_err(|e| e.to_string())?;
    migrate_steps_to_notes(&mut project);
    Ok(Some(project))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_channel_through_xml() {
        let mut project = ProjectFile::default();
        project.channels.items.push(Channel {
            id: 1,
            name: "Kick".to_string(),
            sample_path: "samples/kick.wav".to_string(),
            muted: false,
            soloed: false,
            pan: 0.5,
            volume: 0.8,
            mixer_track: 0,
            color: 0,
        });
        project.content.items.push(PatternContentEntry {
            pattern_id: 1,
            channel_id: 1,
            steps: vec![true, false, false, false],
            notes: vec![],
        });

        let mut body = String::new();
        let ser = quick_xml::se::Serializer::new(&mut body);
        project.serialize(ser).unwrap();
        let back: ProjectFile = quick_xml::de::from_str(&body).unwrap();

        assert_eq!(back.channels.items[0].name, "Kick");
        assert_eq!(back.content.items[0].steps, vec![true, false, false, false]);
    }

    #[test]
    fn migrates_old_steps_into_equivalent_notes() {
        let mut project = ProjectFile::default();
        project.content.items.push(PatternContentEntry {
            pattern_id: 1,
            channel_id: 1,
            steps: vec![true, false, false, true],
            notes: vec![Note { id: 1, pitch: 72, start: 10.0, length: 2.0, velocity: 0.5 }],
        });

        migrate_steps_to_notes(&mut project);

        let entry = &project.content.items[0];
        assert!(entry.steps.is_empty());
        // The pre-existing piano-roll note survives untouched...
        assert!(entry.notes.iter().any(|n| n.id == 1 && n.pitch == 72 && n.start == 10.0));
        // ...alongside the two migrated steps, at the fixed step pitch/length.
        let migrated: Vec<&Note> = entry.notes.iter().filter(|n| n.pitch == STEP_PITCH).collect();
        assert_eq!(migrated.len(), 2);
        assert!(migrated.iter().any(|n| n.start == 0.0 && n.length == 1.0));
        assert!(migrated.iter().any(|n| n.start == 3.0 && n.length == 1.0));
    }

    #[test]
    fn missing_mixer_tracks_and_step_count_default_on_old_files() {
        let old = r#"<ProjectFile>
            <beatsPerBar>4</beatsPerBar><tempo>120</tempo><selectedPatternId>1</selectedPatternId>
            <channels/><content/><patterns/><placements/>
        </ProjectFile>"#;
        let p: ProjectFile = quick_xml::de::from_str(old).unwrap();
        assert_eq!(p.step_count, 16);
        assert!(p.mixer_tracks.items.is_empty());
        assert_eq!(p.master_volume, 1.0);
    }
}
