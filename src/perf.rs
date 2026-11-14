use std::time::Instant;

const LOG_EVERY: usize = 120;

/// Rolling frame-time instrumentation. Call `tick()` once per render; it
/// logs min/avg/max frame time to stderr every `LOG_EVERY` frames so
/// performance can be measured instead of eyeballed.
pub struct FrameStats {
    last_frame: Option<Instant>,
    samples: Vec<f32>,
}

impl FrameStats {
    pub fn new() -> Self {
        Self {
            last_frame: None,
            samples: Vec::with_capacity(LOG_EVERY),
        }
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_frame {
            self.samples.push(now.duration_since(last).as_secs_f32() * 1000.0);
        }
        self.last_frame = Some(now);

        if self.samples.len() >= LOG_EVERY {
            let min = self.samples.iter().cloned().fold(f32::MAX, f32::min);
            let max = self.samples.iter().cloned().fold(f32::MIN, f32::max);
            let avg = self.samples.iter().sum::<f32>() / self.samples.len() as f32;
            eprintln!(
                "frame time (ms) over last {} frames: min={min:.2} avg={avg:.2} max={max:.2} ({:.1} fps avg)",
                self.samples.len(),
                1000.0 / avg,
            );
            self.samples.clear();
        }
    }
}
