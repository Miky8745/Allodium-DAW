use quick_xml::se::Serializer;
use serde::{Deserialize, Serialize};

use crate::projects_root;

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

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChannelSettings {
    pub id: u32,
    pub sample_path: Option<String>,
    #[serde(default)]
    pub sample_folder: Option<String>,
    pub muted: bool,
    // Ctrl+click a channel's mute dot in ChannelRow (Channel Rack only —
    // unrelated to the Mixer's own separate soloed field). #[serde(default)]
    // for backward compat with project files saved before this field existed.
    #[serde(default)]
    pub soloed: bool,
    pub pan: f64,
    pub volume: f64,
    pub mixer_track: u32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    // MIDI output of this channel's piano-roll notes (port key, see
    // midi.rs's port_key) + MIDI channel 1-16. None/"" = no MIDI out.
    #[serde(default)]
    pub midi_out_port: Option<String>,
    #[serde(default = "default_midi_channel")]
    pub midi_out_channel: u8,
}

fn default_midi_channel() -> u8 {
    1
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NoteData {
    pub id: u32,
    pub pitch: i32,
    // f64, not u32: Shift-free editing in the Piano Roll makes start/length
    // fractional. Whole numbers written by older versions still parse.
    pub start: f64,
    pub length: f64,
    pub velocity: f64,
    // Note properties (Piano Roll properties panel). #[serde(default)] so
    // project files saved before they existed still load.
    #[serde(default = "default_half")]
    pub pan: f64,
    #[serde(default = "default_half")]
    pub release: f64,
    #[serde(default = "default_one")]
    pub mod_x: f64,
    #[serde(default)]
    pub mod_y: f64,
    #[serde(default)]
    pub fine_pitch: f64,
    #[serde(default = "default_note_color")]
    pub color: u32,
    #[serde(default)]
    pub slide: bool,
}

fn default_half() -> f64 {
    0.5
}

fn default_one() -> f64 {
    1.0
}

// Lime — must match noteColorStore.svelte.ts's LIME_NOTE_COLOR.
fn default_note_color() -> u32 {
    5
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PatternContentEntry {
    pub pattern_id: u32,
    pub channel_id: u32,
    #[serde(default)]
    pub steps: Vec<bool>,
    #[serde(default)]
    pub notes: Vec<NoteData>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PatternDataXml {
    pub id: u32,
    pub name: String,
    pub color: String,
    pub length_beats: f64,
}

fn default_placement_kind() -> String {
    "pattern".to_string()
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlacementXml {
    pub id: u32,
    // "pattern" | "clip". Deliberately a plain String with sentinel-valued
    // siblings below (never Option<T>) — quick-xml round-trips
    // Option::None as an empty tag that deserializes back as Some(""), not
    // None (see ChannelSettings.sample_path/sample_folder), and an empty
    // tag's content isn't even valid text for a numeric Option, so sentinel
    // defaults sidestep both issues rather than replicating the workaround.
    #[serde(default = "default_placement_kind")]
    pub kind: String,
    #[serde(default)]
    pub pattern_id: u32, // real PatternData id when kind=="pattern"; 0 when kind=="clip"
    pub track_id: u32,
    pub start_beat: f64,
    #[serde(default)]
    pub sample_path: String, // real path when kind=="clip"; "" when kind=="pattern". Absent entirely in project files saved before clip placements existed.
    #[serde(default)]
    pub length_beats: f64, // real clip length when kind=="clip"; 0.0 when kind=="pattern". Same forward-compat note as sample_path.
}

// quick-xml repeats a bare Vec<T> field's own tag name per element rather
// than emitting one tag per item, so each list gets a wrapper struct with a
// renamed inner field to produce clean per-item tags (<channel>, <entry>, …).
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ChannelsXml {
    #[serde(rename = "channel", default)]
    pub items: Vec<ChannelSettings>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ContentXml {
    #[serde(rename = "entry", default)]
    pub items: Vec<PatternContentEntry>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PatternsXml {
    #[serde(rename = "pattern", default)]
    pub items: Vec<PatternDataXml>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PlacementsXml {
    #[serde(rename = "placement", default)]
    pub items: Vec<PlacementXml>,
}

// Piano Roll note palette — 15 hex colors (see noteColorStore.svelte.ts).
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct NoteColorsXml {
    #[serde(rename = "color", default)]
    pub items: Vec<String>,
}

fn default_step_count() -> u32 {
    16
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub beats_per_bar: u32,
    pub pattern_length: u32,
    // How many step-sequencer buttons each channel's steps array has, shared
    // by every channel/pattern (see channelStore.svelte.ts's stepCount).
    // #[serde(default)] for backward compat with project files saved before
    // this field existed — the frontend also treats a missing/zero value
    // the same way (falls back to 16) via the `|| DEFAULT_STEP_COUNT` guard
    // in channelStore.importState().
    #[serde(default = "default_step_count")]
    pub step_count: u32,
    pub tempo: f64,
    pub selected_pattern_id: u32,
    pub channels: ChannelsXml,
    pub content: ContentXml,
    pub patterns: PatternsXml,
    pub placements: PlacementsXml,
    #[serde(default)]
    pub note_colors: NoteColorsXml,
}

// Projects are saved as `<name>.alp`. Older builds wrote `<name>.vlp` (same
// format); load_project still reads one when no .alp exists yet, and the
// next save writes the .alp.
const PROJECT_EXT: &str = "alp";
const LEGACY_PROJECT_EXT: &str = "vlp";

#[tauri::command]
pub fn save_project(name: String, project: ProjectFile) -> Result<(), String> {
    let clean = validate_project_name(&name)?;
    let path = projects_root()?.join(clean).join(format!("{}.{}", clean, PROJECT_EXT));
    let mut body = String::new();
    let mut ser = Serializer::new(&mut body);
    ser.indent(' ', 2);
    project.serialize(ser).map_err(|e| e.to_string())?;
    let xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}\n", body);
    std::fs::write(&path, xml).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_project(name: String) -> Result<Option<ProjectFile>, String> {
    let clean = validate_project_name(&name)?;
    let dir = projects_root()?.join(clean);
    let path = [PROJECT_EXT, LEGACY_PROJECT_EXT]
        .iter()
        .map(|ext| dir.join(format!("{}.{}", clean, ext)))
        .find(|p| p.exists());
    let Some(path) = path else {
        return Ok(None);
    };
    let xml = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    quick_xml::de::from_str(&xml).map(Some).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_history(name: String, contents: String) -> Result<(), String> {
    let clean = validate_project_name(&name)?;
    let path = projects_root()?.join(clean).join("history");
    std::fs::write(&path, contents).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_history(name: String) -> Result<Option<String>, String> {
    let clean = validate_project_name(&name)?;
    let path = projects_root()?.join(clean).join("history");
    if !path.exists() {
        return Ok(None);
    }
    std::fs::read_to_string(&path).map(Some).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_PROJECT: &str = r#"<ProjectFile>
  <beatsPerBar>4</beatsPerBar><patternLength>16</patternLength><tempo>120</tempo><selectedPatternId>1</selectedPatternId>
  <channels/>
  <content><entry><patternId>1</patternId><channelId>0</channelId>
    <notes><id>1</id><pitch>60</pitch><start>4</start><length>2</length><velocity>0.8</velocity></notes>
  </entry></content>
  <patterns/><placements/>
</ProjectFile>"#;

    #[test]
    fn loads_notes_saved_before_note_properties() {
        let p: ProjectFile = quick_xml::de::from_str(OLD_PROJECT).unwrap();
        let n = &p.content.items[0].notes[0];
        assert_eq!((n.start, n.length), (4.0, 2.0));
        assert_eq!((n.pan, n.release, n.mod_x, n.mod_y), (0.5, 0.5, 1.0, 0.0));
        assert_eq!((n.color, n.slide), (5, false));
        assert!(p.note_colors.items.is_empty());
    }

    #[test]
    fn round_trips_fractional_slide_notes_and_palette() {
        let mut p: ProjectFile = quick_xml::de::from_str(OLD_PROJECT).unwrap();
        let n = &mut p.content.items[0].notes[0];
        n.start = 4.5;
        n.slide = true;
        n.color = 7;
        n.fine_pitch = -250.0;
        p.note_colors.items = vec!["#f9fffe".into(), "#123456".into()];
        let mut body = String::new();
        let mut ser = Serializer::new(&mut body);
        ser.indent(' ', 2);
        p.serialize(ser).unwrap();
        let back: ProjectFile = quick_xml::de::from_str(&body).unwrap();
        let n = &back.content.items[0].notes[0];
        assert_eq!((n.start, n.slide, n.color, n.fine_pitch), (4.5, true, 7, -250.0));
        assert_eq!(back.note_colors.items, vec!["#f9fffe".to_string(), "#123456".to_string()]);
    }
}
