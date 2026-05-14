pub struct AudioBuffer {
    pub channels: u16,
    pub sample_rate: u32,
}

impl AudioBuffer {
    pub fn new(channels: u16, sample_rate: u32) -> Self {
        Self {
            channels,
            sample_rate,
        }
    }
}
