//! Decodes a sample file into an in-memory interleaved `f32` PCM buffer,
//! cached once and shared (via `Arc`) across every voice that plays it.

use std::path::Path;
use std::sync::Arc;

use symphonia::core::audio::SampleBuffer as SymSampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub struct SampleBuffer {
    pub channels: u16,
    pub sample_rate: u32,
    /// Interleaved, one `f32` per channel per frame.
    pub data: Vec<f32>,
}

impl SampleBuffer {
    pub fn frame_count(&self) -> u64 {
        (self.data.len() / self.channels.max(1) as usize) as u64
    }
}

/// Some sample libraries ship a `.wav` file that's actually raw Ogg
/// Vorbis data wrapped in a WAV RIFF container (`fmt ` chunk's format
/// tag `0x674F`) — a real case in this app's own sample library
/// (`local_samples/Vocals/*.wav`), not a hypothetical. Ported from
/// Allodium-tauri's `unwrap_ogg_in_wav` in `src-tauri/src/lib.rs`: strip
/// the WAV wrapper and hand back the raw Ogg payload instead, so the
/// probe can be pointed at the right demuxer. A no-op for every normal
/// WAV/PCM file.
fn unwrap_ogg_in_wav(bytes: Vec<u8>) -> (Vec<u8>, bool) {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return (bytes, false);
    }
    let mut pos: usize = 12;
    let mut format_tag: u16 = 0;
    let mut data_chunk: Option<(usize, usize)> = None;
    while pos + 8 <= bytes.len() {
        let chunk_id = &bytes[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let payload_start = pos + 8;
        let payload_end = payload_start.saturating_add(chunk_size).min(bytes.len());
        if chunk_id == b"fmt " && payload_end - payload_start >= 2 {
            format_tag = u16::from_le_bytes([bytes[payload_start], bytes[payload_start + 1]]);
        } else if chunk_id == b"data" {
            data_chunk = Some((payload_start, payload_end - payload_start));
        }
        pos = payload_start.saturating_add(chunk_size).saturating_add(chunk_size % 2);
    }
    if format_tag == 0x674F {
        if let Some((start, len)) = data_chunk {
            return (bytes[start..start + len].to_vec(), true);
        }
    }
    (bytes, false)
}

pub fn decode_sample(path: &Path) -> Result<Arc<SampleBuffer>, String> {
    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    let (bytes, was_ogg_in_wav) = unwrap_ogg_in_wav(raw);

    let mut hint = Hint::new();
    if was_ogg_in_wav {
        hint.with_extension("ogg");
    } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)), Default::default());
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| e.to_string())?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| "no audio track found".to_string())?
        .clone();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;
    let track_id = track.id;

    let mut channels: u16 = 0;
    let mut sample_rate: u32 = 0;
    let mut data: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            // Some decoders report a recoverable "reset required" mid-stream
            // (e.g. a changed Vorbis setup header); retry the packet once.
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        };
        let spec = *decoded.spec();
        channels = spec.channels.count() as u16;
        sample_rate = spec.rate;
        let mut buf = SymSampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        data.extend_from_slice(buf.samples());
    }

    if channels == 0 {
        return Err("no decodable audio frames".to_string());
    }
    Ok(Arc::new(SampleBuffer { channels, sample_rate, data }))
}
