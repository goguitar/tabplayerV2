use anyhow::Result;

use crate::audio_clock::AudioClock;

#[derive(Default)]
pub struct AudioManager {
    pub clock: AudioClock,
}

impl AudioManager {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            clock: AudioClock::new(sample_rate),
        }
    }

    pub fn play(&mut self) {}

    pub fn pause(&mut self) {}

    pub fn seek_seconds(&mut self, seconds: f64) {
        self.clock.seek_seconds(seconds);
    }

    pub fn load_wem_from_path(&mut self, _path: &str) -> Result<()> {
        Ok(())
    }
}
