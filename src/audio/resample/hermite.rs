use crate::audio::buffer::PooledBuffer;

pub struct HermiteResampler {
    ratio: f32,
    index: f32,
    channels: usize,
    last_samples: Vec<i16>,
}

impl HermiteResampler {
    pub fn new(source_rate: u32, target_rate: u32, channels: usize) -> Self {
        Self {
            ratio: source_rate as f32 / target_rate as f32,
            index: 0.0,
            channels,
            last_samples: vec![0; channels],
        }
    }

    #[inline]
    fn hermite(p: [f32; 4], t: f32) -> f32 {
        let c0 = p[1];
        let c1 = 0.5 * (p[2] - p[0]);
        let c2 = p[0] - 2.5 * p[1] + 2.0 * p[2] - 0.5 * p[3];
        let c3 = 0.5 * (p[3] - p[0]) + 1.5 * (p[1] - p[2]);
        ((c3 * t + c2) * t + c1) * t + c0
    }

    pub fn process(&mut self, input: &[i16], output: &mut PooledBuffer) {
        let num_frames = input.len() / self.channels;
        let num_frames_f = num_frames as f32;

        while self.index < num_frames_f {
            let idx = self.index as usize;
            let t = self.index.fract();

            for ch in 0..self.channels {
                let base_idx = idx * self.channels + ch;

                let p0 = if idx == 0 {
                    self.last_samples[ch]
                } else {
                    input[base_idx - self.channels]
                } as f32;

                let p1 = input[base_idx] as f32;

                let p2 = if idx + 1 < num_frames {
                    input[base_idx + self.channels]
                } else {
                    input[(num_frames - 1) * self.channels + ch]
                } as f32;

                let p3 = if idx + 2 < num_frames {
                    input[base_idx + 2 * self.channels]
                } else {
                    input[(num_frames - 1) * self.channels + ch]
                } as f32;

                let s = Self::hermite([p0, p1, p2, p3], t).clamp(i16::MIN as f32, i16::MAX as f32)
                    as i16;
                output.push(s);
            }

            self.index += self.ratio;
        }

        self.index -= num_frames as f32;

        if num_frames > 0 {
            for ch in 0..self.channels {
                self.last_samples[ch] = input[(num_frames - 1) * self.channels + ch];
            }
        }
    }

    pub fn reset(&mut self) {
        self.index = 0.0;
        self.last_samples.fill(0);
    }

    pub fn is_passthrough(&self) -> bool {
        (self.ratio - 1.0).abs() < f32::EPSILON
    }
}
