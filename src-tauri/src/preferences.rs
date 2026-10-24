// Global (not per-project) app preferences, persisted as pretty-printed XML
// at data/preferences.xml. Every struct is #[serde(default)] so a file saved
// by an older build (missing newer fields) still loads, with the missing
// fields falling back to Default. No Option<T> fields on purpose — quick-xml
// round-trips Option::None as Some("") (see project.rs's samplePath note),
// so "unset" is encoded as a sentinel value instead (empty string / 0).

use quick_xml::se::Serializer;
use serde::{Deserialize, Serialize};

use crate::app_root;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct GeneralPrefs {
    pub gui_scale: f64,
    pub latency_hint: String,
    pub sample_rate: u32, // 0 = auto (device default)
    pub lookahead_ms: f64,
    pub tick_ms: f64,
    pub autosave_enabled: bool,
    pub autosave_delay_ms: u32,
    pub undo_limit: u32,
    pub reopen_last_project: bool,
    pub last_project: String,
    pub default_bpm: f64,
    pub default_beats_per_bar: u32,
    pub preview_volume: f64,
    pub preview_length_ms: f64,
    pub qwerty_velocity: u32,
    pub monitor_source: String, // app | system (Toolbar audio monitor input)
}

impl Default for GeneralPrefs {
    fn default() -> Self {
        Self {
            gui_scale: 1.0,
            latency_hint: "interactive".into(),
            sample_rate: 0,
            lookahead_ms: 100.0,
            tick_ms: 25.0,
            autosave_enabled: true,
            autosave_delay_ms: 1000,
            undo_limit: 250,
            reopen_last_project: false,
            last_project: String::new(),
            default_bpm: 120.0,
            default_beats_per_bar: 4,
            preview_volume: 0.8,
            preview_length_ms: 150.0,
            qwerty_velocity: 100,
            monitor_source: "app".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct MidiInputPrefs {
    pub name: String,
    pub enabled: bool,
    pub transpose: i32,
    pub channel: u8, // 0 = omni, else 1-16
    pub note_low: u8,
    pub note_high: u8,
    pub vel_curve: String, // linear | soft | hard | custom
    pub vel_exponent: f64,
    pub vel_offset: i32,
    pub vel_min: u8,
    pub vel_max: u8,
    pub fixed_velocity: u8, // 0 = off
    pub bend_range: f64,    // semitones at full pitch-bend deflection
}

impl Default for MidiInputPrefs {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: false,
            transpose: 0,
            channel: 0,
            note_low: 0,
            note_high: 127,
            vel_curve: "linear".into(),
            vel_exponent: 1.0,
            vel_offset: 0,
            vel_min: 1,
            vel_max: 127,
            fixed_velocity: 0,
            bend_range: 2.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct MidiOutputPrefs {
    pub name: String,
    pub enabled: bool,
    pub latency_ms: f64,
    pub transpose: i32,
    pub vel_scale: f64,
    pub all_notes_off_on_stop: bool,
}

impl Default for MidiOutputPrefs {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: false,
            latency_ms: 0.0,
            transpose: 0,
            vel_scale: 1.0,
            all_notes_off_on_stop: true,
        }
    }
}

// Same per-item-tag wrapper trick as project.rs's ChannelsXml.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct MidiInputsXml {
    #[serde(rename = "input")]
    pub items: Vec<MidiInputPrefs>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct MidiOutputsXml {
    #[serde(rename = "output")]
    pub items: Vec<MidiOutputPrefs>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default, rename = "preferences")]
pub struct Preferences {
    pub general: GeneralPrefs,
    pub midi_inputs: MidiInputsXml,
    pub midi_outputs: MidiOutputsXml,
}

fn prefs_path() -> Result<std::path::PathBuf, String> {
    Ok(app_root()?.join("data").join("preferences.xml"))
}

#[tauri::command]
pub fn load_preferences() -> Result<Option<Preferences>, String> {
    let path = prefs_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let xml = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    quick_xml::de::from_str(&xml).map(Some).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_preferences(prefs: Preferences) -> Result<(), String> {
    let path = prefs_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut body = String::new();
    let mut ser = Serializer::new(&mut body);
    ser.indent(' ', 2);
    prefs.serialize(ser).map_err(|e| e.to_string())?;
    let xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}\n", body);
    std::fs::write(&path, xml).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_xml() {
        let mut prefs = Preferences::default();
        prefs.general.gui_scale = 1.25;
        prefs.midi_inputs.items.push(MidiInputPrefs { name: "KeyLab:KeyLab MIDI 1".into(), enabled: true, transpose: -12, ..Default::default() });
        prefs.midi_outputs.items.push(MidiOutputPrefs { name: "Synth".into(), latency_ms: 4.5, ..Default::default() });
        let mut body = String::new();
        let mut ser = Serializer::new(&mut body);
        ser.indent(' ', 2);
        prefs.serialize(ser).unwrap();
        let back: Preferences = quick_xml::de::from_str(&body).unwrap();
        assert_eq!(back.general.gui_scale, 1.25);
        assert_eq!(back.general.last_project, "");
        assert_eq!(back.midi_inputs.items.len(), 1);
        assert_eq!(back.midi_inputs.items[0].transpose, -12);
        assert_eq!(back.midi_outputs.items[0].latency_ms, 4.5);

        let empty: Preferences = quick_xml::de::from_str(&{
            let mut b = String::new();
            Preferences::default().serialize(Serializer::new(&mut b)).unwrap();
            b
        }).unwrap();
        assert!(empty.midi_inputs.items.is_empty());

        // Older file missing most fields still loads with defaults.
        let partial: Preferences = quick_xml::de::from_str("<preferences><general><guiScale>2</guiScale></general></preferences>").unwrap();
        assert_eq!(partial.general.gui_scale, 2.0);
        assert_eq!(partial.general.undo_limit, 250);
    }
}
