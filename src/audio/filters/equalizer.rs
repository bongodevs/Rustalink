use super::AudioFilter;

const BAND_COUNT: usize = 15;
const DEFAULT_MAKEUP_GAIN: f32 = 4.0;

struct Coefficients {
    beta: f32,
    alpha: f32,
    gamma: f32,
}

const COEFFICIENTS_48000: [Coefficients; BAND_COUNT] = [
    Coefficients {
        beta: 0.9984755,
        alpha: 0.0007622667,
        gamma: 1.9984648,
    },
    Coefficients {
        beta: 0.9975618,
        alpha: 0.0012190767,
        gamma: 1.9975345,
    },
    Coefficients {
        beta: 0.9961626,
        alpha: 0.0019186931,
        gamma: 1.9960947,
    },
    Coefficients {
        beta: 0.9939158,
        alpha: 0.0030421073,
        gamma: 1.993745,
    },
    Coefficients {
        beta: 0.9902831,
        alpha: 0.004858464,
        gamma: 1.9898466,
    },
    Coefficients {
        beta: 0.984859,
        alpha: 0.0075705137,
        gamma: 1.9837963,
    },
    Coefficients {
        beta: 0.9758851,
        alpha: 0.012057437,
        gamma: 1.9731772,
    },
    Coefficients {
        beta: 0.9622852,
        alpha: 0.018857391,
        gamma: 1.9556165,
    },
    Coefficients {
        beta: 0.9408093,
        alpha: 0.029595335,
        gamma: 1.9242054,
    },
    Coefficients {
        beta: 0.9070206,
        alpha: 0.046489704,
        gamma: 1.8653476,
    },
    Coefficients {
        beta: 0.85868,
        alpha: 0.07065998,
        gamma: 1.7600402,
    },
    Coefficients {
        beta: 0.7840961,
        alpha: 0.10795194,
        gamma: 1.5450726,
    },
    Coefficients {
        beta: 0.6833286,
        alpha: 0.1583357,
        gamma: 1.1426447,
    },
    Coefficients {
        beta: 0.5526752,
        alpha: 0.2236624,
        gamma: 0.4018619,
    },
    Coefficients {
        beta: 0.41811886,
        alpha: 0.29094055,
        gamma: -0.7090594,
    },
];

#[derive(Clone, Default)]
struct EqBandState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl EqBandState {
    fn process(&mut self, sample: f32, coeffs: &Coefficients) -> f32 {
        let result =
            coeffs.alpha * (sample - self.x2) + coeffs.gamma * self.y1 - coeffs.beta * self.y2;

        self.x2 = self.x1;
        self.x1 = sample;
        self.y2 = self.y1;

        if !result.is_finite() {
            self.y1 = 0.0;
            return 0.0;
        }

        self.y1 = result;
        result
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

pub struct EqualizerFilter {
    gains: [f32; BAND_COUNT],
    states: [[EqBandState; 2]; BAND_COUNT],
    makeup_gain: f32,
}

impl EqualizerFilter {
    pub fn new(bands: &[(u8, f32)]) -> Self {
        let mut gains = [0.0f32; BAND_COUNT];
        for &(band, gain) in bands {
            if (band as usize) < BAND_COUNT {
                gains[band as usize] = gain.clamp(-0.25, 1.0);
            }
        }

        let states: [[EqBandState; 2]; BAND_COUNT] =
            std::array::from_fn(|_| [EqBandState::default(), EqBandState::default()]);

        let positive_sum: f32 = gains.iter().filter(|g| **g > 0.0).sum();
        let makeup_gain = if positive_sum > 1.0 {
            DEFAULT_MAKEUP_GAIN / (1.0 + (positive_sum - 1.0) * 0.5)
        } else {
            DEFAULT_MAKEUP_GAIN
        };

        Self {
            gains,
            states,
            makeup_gain,
        }
    }
}

impl AudioFilter for EqualizerFilter {
    fn process(&mut self, samples: &mut [i16]) {
        let num_frames = samples.len() / 2;

        for frame in 0..num_frames {
            let offset = frame * 2;

            let left_f = samples[offset] as f32 / 32768.0;
            let right_f = samples[offset + 1] as f32 / 32768.0;

            let mut result_left = left_f * 0.25;
            let mut result_right = right_f * 0.25;

            for (b, coeffs) in COEFFICIENTS_48000.iter().enumerate() {
                let gain = self.gains[b];
                if gain.abs() < f32::EPSILON {
                    self.states[b][0].process(left_f, coeffs);
                    self.states[b][1].process(right_f, coeffs);
                    continue;
                }

                let band_left = self.states[b][0].process(left_f, coeffs);
                let band_right = self.states[b][1].process(right_f, coeffs);

                result_left += band_left * gain;
                result_right += band_right * gain;
            }

            let out_left = (result_left * self.makeup_gain).tanh();
            let out_right = (result_right * self.makeup_gain).tanh();

            samples[offset] = (out_left * 32767.0).round() as i16;
            samples[offset + 1] = (out_right * 32767.0).round() as i16;
        }
    }

    fn is_enabled(&self) -> bool {
        self.gains.iter().any(|g| g.abs() > f32::EPSILON)
    }

    fn reset(&mut self) {
        for band_states in self.states.iter_mut() {
            for state in band_states.iter_mut() {
                state.reset();
            }
        }
    }
}
