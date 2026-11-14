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

/// A channel's step/note content for one specific pattern. Channels are
/// shared across all patterns; their actual step/note data is stashed
/// per pattern here instead, mirroring Allodium-tauri's
/// "pattern-scoped content" model (see `docs/audio-playback.md` there).
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

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Pattern {
    pub id: u32,
    #[serde(default)]
    pub name: String,
    pub length_beats: f64,
}

/// A pattern block placed on a Playlist track. No clip (audio-file)
/// placements for the MVP — patterns only, so unlike the original
/// there's no `kind` sentinel to distinguish the two. Length comes from
/// the referenced `Pattern.length_beats` rather than being duplicated
/// here.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub id: u32,
    pub pattern_id: u32,
    pub track_id: u32,
    pub start_beat: f64,
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
    quick_xml::de::from_str(&xml).map(Some).map_err(|e| e.to_string())
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
    fn missing_mixer_tracks_and_step_count_default_on_old_files() {
        let old = r#"<ProjectFile>
            <beatsPerBar>4</beatsPerBar><tempo>120</tempo><selectedPatternId>1</selectedPatternId>
            <channels/><content/><patterns/><placements/>
        </ProjectFile>"#;
        let p: ProjectFile = quick_xml::de::from_str(old).unwrap();
        assert_eq!(p.step_count, 16);
        assert!(p.mixer_tracks.items.is_empty());
    }
}
