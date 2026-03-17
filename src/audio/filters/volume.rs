use super::AudioFilter;

/// Volume filter.
pub struct VolumeFilter {
    volume: f32,
}

impl VolumeFilter {
    pub fn new(volume: f32) -> Self {
        Self { volume }
    }
}

impl AudioFilter for VolumeFilter {
    fn process(&mut self, samples: &mut [i16]) {
        let vol = self.volume;
        for sample in samples.iter_mut() {
            *sample = (*sample as f32 * vol).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        }
    }

    fn is_enabled(&self) -> bool {
        (self.volume - 1.0).abs() > f32::EPSILON
    }

    fn reset(&mut self) {}
}
