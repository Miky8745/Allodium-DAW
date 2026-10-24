// MIDI I/O via midir (ALSA on Linux). Lives in Rust because WebKitGTK has
// no Web MIDI API. Ports are identified by a stable "key" — the backend's
// port name with ALSA's trailing "client:port" numbers stripped — since
// those numbers change between replugs and the frontend remembers devices
// (and their per-device settings) by key across sessions.
//
// Incoming messages are forwarded as `midi-message` events; port-list
// changes (hot-plug, polled every 2s) as `midi-ports-changed`. Outgoing
// notes are either sent immediately (`midi_send`) or queued with a delay
// (`midi_schedule`) on a scheduler thread, so the frontend's look-ahead
// audio scheduler can hand over note-on/off pairs ahead of time.

use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

const CLIENT_NAME: &str = "Allodium";
const HOTPLUG_POLL: Duration = Duration::from_secs(2);

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MidiPorts {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub connected_inputs: Vec<String>,
    pub connected_outputs: Vec<String>,
}

#[derive(Serialize, Clone)]
struct MidiMessageEvent {
    port: String,
    bytes: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledEvent {
    pub port: String,
    pub bytes: Vec<u8>,
    pub delay_ms: f64,
}

struct Queued {
    at: Instant,
    seq: u64,
    port: String,
    bytes: Vec<u8>,
}
impl PartialEq for Queued {
    fn eq(&self, o: &Self) -> bool { self.at == o.at && self.seq == o.seq }
}
impl Eq for Queued {}
impl PartialOrd for Queued {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(o)) }
}
impl Ord for Queued {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering { (self.at, self.seq).cmp(&(o.at, o.seq)) }
}

#[derive(Default)]
struct Inner {
    inputs: HashMap<String, MidiInputConnection<()>>,
    outputs: HashMap<String, MidiOutputConnection>,
    // (channel 0-15, note) pairs that got a note-on with no note-off yet,
    // per output — what midi_stop_all() silences after clearing the queue.
    active_notes: HashMap<String, HashSet<(u8, u8)>>,
}

impl Inner {
    fn send(&mut self, port: &str, bytes: &[u8]) {
        let Some(conn) = self.outputs.get_mut(port) else { return };
        if conn.send(bytes).is_err() || bytes.len() < 3 { return; }
        let status = bytes[0] & 0xF0;
        let key = (bytes[0] & 0x0F, bytes[1]);
        let active = self.active_notes.entry(port.to_string()).or_default();
        match status {
            0x90 if bytes[2] > 0 => { active.insert(key); }
            0x80 | 0x90 => { active.remove(&key); }
            _ => {}
        }
    }
}

pub struct MidiState {
    inner: Mutex<Inner>,
    queue: Mutex<(BinaryHeap<Reverse<Queued>>, u64)>,
    queue_cv: Condvar,
    last_ports: Mutex<Option<MidiPorts>>,
}

pub type SharedMidi = Arc<MidiState>;

pub fn new_state() -> SharedMidi {
    Arc::new(MidiState {
        inner: Mutex::new(Inner::default()),
        queue: Mutex::new((BinaryHeap::new(), 0)),
        queue_cv: Condvar::new(),
        last_ports: Mutex::new(None),
    })
}

// "Arturia KeyLab:Arturia KeyLab MIDI 1 24:0" -> "Arturia KeyLab:Arturia KeyLab MIDI 1"
fn port_key(name: &str) -> String {
    let trimmed = name.trim_end();
    if let Some((head, tail)) = trimmed.rsplit_once(' ') {
        let mut parts = tail.split(':');
        let numeric = |p: Option<&str>| p.map_or(false, |s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()));
        if numeric(parts.next()) && numeric(parts.next()) && parts.next().is_none() {
            return head.to_string();
        }
    }
    trimmed.to_string()
}

// Deduplicates identical keys (two units of the same controller) with a
// " #2"/" #3" suffix, in backend enumeration order.
fn keyed<P>(names: Vec<(String, P)>) -> Vec<(String, P)> {
    let mut seen: HashMap<String, u32> = HashMap::new();
    names
        .into_iter()
        .filter(|(n, _)| !n.starts_with(CLIENT_NAME))
        .map(|(n, p)| {
            let base = port_key(&n);
            let count = seen.entry(base.clone()).or_insert(0);
            *count += 1;
            let key = if *count == 1 { base } else { format!("{} #{}", base, count) };
            (key, p)
        })
        .collect()
}

fn input_ports() -> Vec<(String, midir::MidiInputPort)> {
    let Ok(mi) = MidiInput::new(CLIENT_NAME) else { return vec![] };
    let named = mi.ports().into_iter().filter_map(|p| mi.port_name(&p).ok().map(|n| (n, p))).collect();
    keyed(named)
}

fn output_ports() -> Vec<(String, midir::MidiOutputPort)> {
    let Ok(mo) = MidiOutput::new(CLIENT_NAME) else { return vec![] };
    let named = mo.ports().into_iter().filter_map(|p| mo.port_name(&p).ok().map(|n| (n, p))).collect();
    keyed(named)
}

fn snapshot(state: &MidiState) -> MidiPorts {
    let inputs: Vec<String> = input_ports().into_iter().map(|(k, _)| k).collect();
    let outputs: Vec<String> = output_ports().into_iter().map(|(k, _)| k).collect();
    snapshot_with(state, inputs, outputs)
}

fn snapshot_with(state: &MidiState, inputs: Vec<String>, outputs: Vec<String>) -> MidiPorts {
    let inner = state.inner.lock().unwrap();
    let mut connected_inputs: Vec<String> = inner.inputs.keys().cloned().collect();
    let mut connected_outputs: Vec<String> = inner.outputs.keys().cloned().collect();
    connected_inputs.sort();
    connected_outputs.sort();
    MidiPorts { inputs, outputs, connected_inputs, connected_outputs }
}

// Background threads: the hot-plug poller and the delayed-send scheduler.
pub fn start_threads(app: AppHandle, state: SharedMidi) {
    let poll_state = state.clone();
    std::thread::spawn(move || loop {
        let inputs: Vec<String> = input_ports().into_iter().map(|(k, _)| k).collect();
        let outputs: Vec<String> = output_ports().into_iter().map(|(k, _)| k).collect();
        // Drop connections whose device vanished so that re-enabling (the
        // frontend re-applies its enabled list on every change) reconnects
        // to the freshly re-enumerated port instead of a dead handle.
        {
            let mut inner = poll_state.inner.lock().unwrap();
            inner.inputs.retain(|k, _| inputs.contains(k));
            inner.outputs.retain(|k, _| outputs.contains(k));
            let outs = inner.outputs.keys().cloned().collect::<HashSet<_>>();
            inner.active_notes.retain(|k, _| outs.contains(k));
        }
        let ports = snapshot_with(&poll_state, inputs, outputs);
        let mut last = poll_state.last_ports.lock().unwrap();
        if last.as_ref() != Some(&ports) {
            *last = Some(ports.clone());
            let _ = app.emit("midi-ports-changed", ports);
        }
        drop(last);
        std::thread::sleep(HOTPLUG_POLL);
    });

    std::thread::spawn(move || {
        let mut guard = state.queue.lock().unwrap();
        loop {
            let now = Instant::now();
            let mut due = Vec::new();
            while guard.0.peek().map_or(false, |Reverse(q)| q.at <= now) {
                due.push(guard.0.pop().unwrap().0);
            }
            if !due.is_empty() {
                drop(guard);
                let mut inner = state.inner.lock().unwrap();
                for q in due { inner.send(&q.port, &q.bytes); }
                drop(inner);
                guard = state.queue.lock().unwrap();
                continue;
            }
            guard = match guard.0.peek().map(|Reverse(q)| q.at) {
                Some(at) => state.queue_cv.wait_timeout(guard, at.saturating_duration_since(now)).unwrap().0,
                None => state.queue_cv.wait(guard).unwrap(),
            };
        }
    });
}

#[tauri::command]
pub fn midi_list_ports(state: State<'_, SharedMidi>) -> MidiPorts {
    snapshot(&state)
}

#[tauri::command]
pub fn midi_set_inputs(app: AppHandle, state: State<'_, SharedMidi>, names: Vec<String>) -> MidiPorts {
    let wanted: HashSet<String> = names.into_iter().collect();
    {
        let mut inner = state.inner.lock().unwrap();
        inner.inputs.retain(|k, _| wanted.contains(k));
        for (key, port) in input_ports() {
            if !wanted.contains(&key) || inner.inputs.contains_key(&key) { continue; }
            let Ok(mut mi) = MidiInput::new(CLIENT_NAME) else { continue };
            // MIDI clock/active sensing would flood the event bridge for no use.
            mi.ignore(Ignore::TimeAndActiveSense);
            let app = app.clone();
            let port_name = key.clone();
            let conn = mi.connect(&port, "in", move |_ts, bytes, _| {
                let _ = app.emit("midi-message", MidiMessageEvent { port: port_name.clone(), bytes: bytes.to_vec() });
            }, ());
            if let Ok(conn) = conn { inner.inputs.insert(key, conn); }
        }
    }
    snapshot(&state)
}

#[tauri::command]
pub fn midi_set_outputs(state: State<'_, SharedMidi>, names: Vec<String>) -> MidiPorts {
    let wanted: HashSet<String> = names.into_iter().collect();
    {
        let mut inner = state.inner.lock().unwrap();
        let removed: Vec<String> = inner.outputs.keys().filter(|k| !wanted.contains(*k)).cloned().collect();
        for key in removed {
            silence(&mut inner, &key, false);
            inner.outputs.remove(&key);
            inner.active_notes.remove(&key);
        }
        for (key, port) in output_ports() {
            if !wanted.contains(&key) || inner.outputs.contains_key(&key) { continue; }
            let Ok(mo) = MidiOutput::new(CLIENT_NAME) else { continue };
            if let Ok(conn) = mo.connect(&port, "out") { inner.outputs.insert(key, conn); }
        }
    }
    snapshot(&state)
}

#[tauri::command]
pub fn midi_send(state: State<'_, SharedMidi>, port: String, bytes: Vec<u8>) {
    state.inner.lock().unwrap().send(&port, &bytes);
}

#[tauri::command]
pub fn midi_schedule(state: State<'_, SharedMidi>, events: Vec<ScheduledEvent>) {
    let now = Instant::now();
    let mut guard = state.queue.lock().unwrap();
    for e in events {
        guard.1 += 1;
        let seq = guard.1;
        let at = now + Duration::from_secs_f64(e.delay_ms.max(0.0) / 1000.0);
        guard.0.push(Reverse(Queued { at, seq, port: e.port, bytes: e.bytes }));
    }
    drop(guard);
    state.queue_cv.notify_one();
}

fn silence(inner: &mut Inner, port: &str, cc123: bool) {
    let notes: Vec<(u8, u8)> = inner.active_notes.get(port).map(|s| s.iter().copied().collect()).unwrap_or_default();
    for (ch, note) in notes { inner.send(port, &[0x80 | ch, note, 0]); }
    if cc123 {
        for ch in 0..16u8 { inner.send(port, &[0xB0 | ch, 123, 0]); }
    }
}

// Transport stop: drop every not-yet-sent queued event, then note-off every
// note that actually went out (a dropped queued note-off would otherwise
// leave it stuck), plus CC123 "all notes off" on the given ports.
#[tauri::command]
pub fn midi_stop_all(state: State<'_, SharedMidi>, cc123_ports: Vec<String>) {
    state.queue.lock().unwrap().0.clear();
    let mut inner = state.inner.lock().unwrap();
    let ports: Vec<String> = inner.outputs.keys().cloned().collect();
    for port in ports {
        let cc = cc123_ports.contains(&port);
        silence(&mut inner, &port, cc);
    }
}

#[cfg(test)]
mod tests {
    use super::port_key;

    #[test]
    fn strips_alsa_client_port_numbers() {
        assert_eq!(port_key("Arturia KeyLab:Arturia KeyLab MIDI 1 24:0"), "Arturia KeyLab:Arturia KeyLab MIDI 1");
        assert_eq!(port_key("Midi Through:Midi Through Port-0 14:0"), "Midi Through:Midi Through Port-0");
        assert_eq!(port_key("Plain name"), "Plain name");
        assert_eq!(port_key("Ends with 3:x"), "Ends with 3:x");
    }
}
