#[derive(Debug, Clone, Copy, Default)]
pub struct AudioClock {
    pub current_sample: u64,
    pub sample_rate: u32,
}

impl AudioClock {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            current_sample: 0,
            sample_rate,
        }
    }

    pub fn song_time_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.current_sample as f64 / self.sample_rate as f64
    }

    pub fn seek_seconds(&mut self, seconds: f64) {
        let clamped = if seconds.is_sign_negative() { 0.0 } else { seconds };
        self.current_sample = (clamped * self.sample_rate as f64) as u64;
    }
}
