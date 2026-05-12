#[derive(Debug, Clone, Copy, Default)]
pub struct SongClock {
    pub current_sample: u64,
    pub sample_rate: u32,
    pub speed: f64,
    pub playing: bool,
}

impl SongClock {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            current_sample: 0,
            sample_rate,
            speed: 1.0,
            playing: false,
        }
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn seek_seconds(&mut self, seconds: f64) {
        let clamped = if seconds.is_sign_negative() { 0.0 } else { seconds };
        self.current_sample = (clamped * self.sample_rate as f64) as u64;
    }

    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.clamp(0.1, 4.0);
    }

    pub fn tick(&mut self, delta_seconds: f64) {
        if !self.playing || self.sample_rate == 0 {
            return;
        }
        let step = delta_seconds * self.sample_rate as f64 * self.speed;
        self.current_sample = self.current_sample.saturating_add(step.max(0.0) as u64);
    }

    pub fn song_time_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.current_sample as f64 / self.sample_rate as f64
    }
}
