pub mod biquad;
pub mod channel_mix;
pub mod chorus;
pub mod compressor;
pub mod delay_line;
pub mod distortion;
pub mod echo;
pub mod equalizer;
pub mod flanger;
pub mod high_pass;
pub mod karaoke;
pub mod lfo;
pub mod low_pass;
pub mod normalization;
pub mod phaser;
pub mod phonograph;
pub mod reverb;
pub mod rotation;
pub mod spatial;
pub mod timescale;
pub mod tremolo;
pub mod vibrato;
pub mod volume;

use crate::{
    config::FiltersConfig,
    player::{EqBand, Filters},
};

/// Validate if the requested filters are allowed by the server configuration.
/// Returns a list of disabled filter names that were requested.
pub fn validate_filters(filters: &Filters, config: &FiltersConfig) -> Vec<&'static str> {
    let mut invalid = Vec::new();

    if filters.volume.is_some() && !config.volume {
        invalid.push("volume");
    }
    if filters.equalizer.is_some() && !config.equalizer {
        invalid.push("equalizer");
    }
    if filters.karaoke.is_some() && !config.karaoke {
        invalid.push("karaoke");
    }
    if filters.timescale.is_some() && !config.timescale {
        invalid.push("timescale");
    }
    if filters.tremolo.is_some() && !config.tremolo {
        invalid.push("tremolo");
    }
    if filters.vibrato.is_some() && !config.vibrato {
        invalid.push("vibrato");
    }
    if filters.distortion.is_some() && !config.distortion {
        invalid.push("distortion");
    }
    if filters.rotation.is_some() && !config.rotation {
        invalid.push("rotation");
    }
    if filters.channel_mix.is_some() && !config.channel_mix {
        invalid.push("channelMix");
    }
    if filters.low_pass.is_some() && !config.low_pass {
        invalid.push("lowPass");
    }
    if filters.echo.is_some() && !config.echo {
        invalid.push("echo");
    }
    if filters.high_pass.is_some() && !config.high_pass {
        invalid.push("highPass");
    }
    if filters.normalization.is_some() && !config.normalization {
        invalid.push("normalization");
    }
    if filters.chorus.is_some() && !config.chorus {
        invalid.push("chorus");
    }
    if filters.compressor.is_some() && !config.compressor {
        invalid.push("compressor");
    }
    if filters.flanger.is_some() && !config.flanger {
        invalid.push("flanger");
    }
    if filters.phaser.is_some() && !config.phaser {
        invalid.push("phaser");
    }
    if filters.phonograph.is_some() && !config.phonograph {
        invalid.push("phonograph");
    }
    if filters.reverb.is_some() && !config.reverb {
        invalid.push("reverb");
    }
    if filters.spatial.is_some() && !config.spatial {
        invalid.push("spatial");
    }

    invalid
}

/// Trait for audio filters that process interleaved stereo i16 PCM samples.
/// Buffer layout: [L, R, L, R, ...] — 960 frames × 2 channels = 1920 samples per 20ms.
pub trait AudioFilter: Send {
    fn process(&mut self, samples: &mut [i16]);
    fn is_enabled(&self) -> bool;
    fn reset(&mut self);
}

/// Concrete enum of all supported in-place audio filters.
/// This enables the compiler to inline the `process` calls and avoid vtable
/// dispatch overhead (H5).
pub enum ConcreteFilter {
    Volume(volume::VolumeFilter),
    Equalizer(Box<equalizer::EqualizerFilter>),
    Karaoke(Box<karaoke::KaraokeFilter>),
    Tremolo(Box<tremolo::TremoloFilter>),
    Vibrato(Box<vibrato::VibratoFilter>),
    Rotation(Box<rotation::RotationFilter>),
    Distortion(Box<distortion::DistortionFilter>),
    ChannelMix(Box<channel_mix::ChannelMixFilter>),
    LowPass(Box<low_pass::LowPassFilter>),
    Echo(Box<echo::EchoFilter>),
    HighPass(Box<high_pass::HighPassFilter>),
    Normalization(Box<normalization::NormalizationFilter>),
    Chorus(Box<chorus::ChorusFilter>),
    Compressor(Box<compressor::CompressorFilter>),
    Flanger(Box<flanger::FlangerFilter>),
    Phaser(Box<phaser::PhaserFilter>),
    Phonograph(Box<phonograph::PhonographFilter>),
    Reverb(Box<reverb::ReverbFilter>),
    Spatial(Box<spatial::SpatialFilter>),
}

impl ConcreteFilter {
    #[inline(always)]
    pub fn process(&mut self, samples: &mut [i16]) {
        match self {
            Self::Volume(f) => f.process(samples),
            Self::Equalizer(f) => f.process(samples),
            Self::Karaoke(f) => f.process(samples),
            Self::Tremolo(f) => f.process(samples),
            Self::Vibrato(f) => f.process(samples),
            Self::Rotation(f) => f.process(samples),
            Self::Distortion(f) => f.process(samples),
            Self::ChannelMix(f) => f.process(samples),
            Self::LowPass(f) => f.process(samples),
            Self::Echo(f) => f.process(samples),
            Self::HighPass(f) => f.process(samples),
            Self::Normalization(f) => f.process(samples),
            Self::Chorus(f) => f.process(samples),
            Self::Compressor(f) => f.process(samples),
            Self::Flanger(f) => f.process(samples),
            Self::Phaser(f) => f.process(samples),
            Self::Phonograph(f) => f.process(samples),
            Self::Reverb(f) => f.process(samples),
            Self::Spatial(f) => f.process(samples),
        }
    }

    pub fn reset(&mut self) {
        match self {
            Self::Volume(f) => f.reset(),
            Self::Equalizer(f) => f.reset(),
            Self::Karaoke(f) => f.reset(),
            Self::Tremolo(f) => f.reset(),
            Self::Vibrato(f) => f.reset(),
            Self::Rotation(f) => f.reset(),
            Self::Distortion(f) => f.reset(),
            Self::ChannelMix(f) => f.reset(),
            Self::LowPass(f) => f.reset(),
            Self::Echo(f) => f.reset(),
            Self::HighPass(f) => f.reset(),
            Self::Normalization(f) => f.reset(),
            Self::Chorus(f) => f.reset(),
            Self::Compressor(f) => f.reset(),
            Self::Flanger(f) => f.reset(),
            Self::Phaser(f) => f.reset(),
            Self::Phonograph(f) => f.reset(),
            Self::Reverb(f) => f.reset(),
            Self::Spatial(f) => f.reset(),
        }
    }
}

/// An ordered chain of audio filters, constructed from Rustalink API `Filters`.
pub struct FilterChain {
    filters: Vec<ConcreteFilter>,
    /// Timescale filter handled separately (changes buffer length).
    timescale: Option<timescale::TimescaleFilter>,
    /// Residual buffer for timescale output (feeds fixed-size 1920-sample frames).
    timescale_buffer: Vec<i16>,
}

impl FilterChain {
    /// Build a filter chain from the Rustalink API `Filters` config.
    pub fn from_config(config: &Filters) -> Self {
        let mut filters: Vec<ConcreteFilter> = Vec::new();

        // Volume (applied first)
        if let Some(vol) = config.volume {
            let f = volume::VolumeFilter::new(vol);
            if f.is_enabled() {
                filters.push(ConcreteFilter::Volume(f));
            }
        }

        // Equalizer
        if let Some(ref bands) = config.equalizer {
            let band_tuples: Vec<(u8, f32)> =
                bands.iter().map(|b: &EqBand| (b.band, b.gain)).collect();
            let f = equalizer::EqualizerFilter::new(&band_tuples);
            if f.is_enabled() {
                filters.push(ConcreteFilter::Equalizer(Box::new(f)));
            }
        }

        // Karaoke
        if let Some(ref k) = config.karaoke {
            let f = karaoke::KaraokeFilter::new(
                k.level.unwrap_or(1.0),
                k.mono_level.unwrap_or(1.0),
                k.filter_band.unwrap_or(220.0),
                k.filter_width.unwrap_or(100.0),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Karaoke(Box::new(f)));
            }
        }

        // Tremolo
        if let Some(ref t) = config.tremolo {
            let f = tremolo::TremoloFilter::new(t.frequency.unwrap_or(2.0), t.depth.unwrap_or(0.5));
            if f.is_enabled() {
                filters.push(ConcreteFilter::Tremolo(Box::new(f)));
            }
        }

        // Vibrato
        if let Some(ref v) = config.vibrato {
            let f = vibrato::VibratoFilter::new(v.frequency.unwrap_or(2.0), v.depth.unwrap_or(0.5));
            if f.is_enabled() {
                filters.push(ConcreteFilter::Vibrato(Box::new(f)));
            }
        }

        // Rotation
        if let Some(ref r) = config.rotation {
            let f = rotation::RotationFilter::new(r.rotation_hz.unwrap_or(0.0));
            if f.is_enabled() {
                filters.push(ConcreteFilter::Rotation(Box::new(f)));
            }
        }

        // Distortion
        if let Some(ref d) = config.distortion {
            let config = distortion::DistortionConfig {
                sin_offset: d.sin_offset.unwrap_or(0.0),
                sin_scale: d.sin_scale.unwrap_or(1.0),
                cos_offset: d.cos_offset.unwrap_or(0.0),
                cos_scale: d.cos_scale.unwrap_or(1.0),
                tan_offset: d.tan_offset.unwrap_or(0.0),
                tan_scale: d.tan_scale.unwrap_or(1.0),
                offset: d.offset.unwrap_or(0.0),
                scale: d.scale.unwrap_or(1.0),
            };
            let f = distortion::DistortionFilter::new(config);
            if f.is_enabled() {
                filters.push(ConcreteFilter::Distortion(Box::new(f)));
            }
        }

        // Channel Mix
        if let Some(ref cm) = config.channel_mix {
            let f = channel_mix::ChannelMixFilter::new(
                cm.left_to_left.unwrap_or(1.0),
                cm.left_to_right.unwrap_or(0.0),
                cm.right_to_left.unwrap_or(0.0),
                cm.right_to_right.unwrap_or(1.0),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::ChannelMix(Box::new(f)));
            }
        }

        // Low Pass
        if let Some(ref lp) = config.low_pass {
            let f = low_pass::LowPassFilter::new(lp.smoothing.unwrap_or(20.0));
            if f.is_enabled() {
                filters.push(ConcreteFilter::LowPass(Box::new(f)));
            }
        }

        // Echo
        if let Some(ref e) = config.echo {
            let f = echo::EchoFilter::new(e.echo_length.unwrap_or(1.0), e.decay.unwrap_or(0.5));
            if f.is_enabled() {
                filters.push(ConcreteFilter::Echo(Box::new(f)));
            }
        }

        // High Pass
        if let Some(ref hp) = config.high_pass {
            let f = high_pass::HighPassFilter::new(
                hp.cutoff_frequency.unwrap_or(200),
                hp.boost_factor.unwrap_or(1.0),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::HighPass(Box::new(f)));
            }
        }

        // Normalization
        if let Some(ref n) = config.normalization {
            let f = normalization::NormalizationFilter::new(
                n.max_amplitude.unwrap_or(1.0),
                n.adaptive.unwrap_or(true),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Normalization(Box::new(f)));
            }
        }

        // Chorus
        if let Some(ref c) = config.chorus {
            let f = chorus::ChorusFilter::new(
                c.rate.unwrap_or(1.5),
                c.depth.unwrap_or(1.0),
                c.delay.unwrap_or(2.0),
                c.mix.unwrap_or(0.5),
                c.feedback.unwrap_or(0.5),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Chorus(Box::new(f)));
            }
        }

        // Compressor
        if let Some(ref c) = config.compressor {
            let f = compressor::CompressorFilter::new(
                c.threshold.unwrap_or(-10.0),
                c.ratio.unwrap_or(2.0),
                c.attack.unwrap_or(5.0),
                c.release.unwrap_or(50.0),
                c.makeup_gain.unwrap_or(0.0),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Compressor(Box::new(f)));
            }
        }

        // Flanger
        if let Some(ref fl) = config.flanger {
            let f = flanger::FlangerFilter::new(
                fl.rate.unwrap_or(0.2),
                fl.depth.unwrap_or(1.0),
                fl.feedback.unwrap_or(0.5),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Flanger(Box::new(f)));
            }
        }

        // Phaser
        if let Some(ref p) = config.phaser {
            let config = phaser::PhaserConfig {
                stages: p.stages.unwrap_or(4),
                rate: p.rate.unwrap_or(0.0),
                depth: p.depth.unwrap_or(1.0),
                feedback: p.feedback.unwrap_or(0.0),
                mix: p.mix.unwrap_or(0.5),
                min_frequency: p.min_frequency.unwrap_or(100.0),
                max_frequency: p.max_frequency.unwrap_or(2500.0),
            };
            let f = phaser::PhaserFilter::new(config);
            if f.is_enabled() {
                filters.push(ConcreteFilter::Phaser(Box::new(f)));
            }
        }

        // Phonograph
        if let Some(ref ph) = config.phonograph {
            let config = phonograph::PhonographConfig {
                frequency: ph.frequency.unwrap_or(0.8),
                depth: ph.depth.unwrap_or(0.25),
                crackle: ph.crackle.unwrap_or(0.18),
                flutter: ph.flutter.unwrap_or(0.18),
                room: ph.room.unwrap_or(0.22),
                mic_agc: ph.mic_agc.unwrap_or(0.25),
                drive: ph.drive.unwrap_or(0.25),
            };
            let f = phonograph::PhonographFilter::new(config);
            if f.is_enabled() {
                filters.push(ConcreteFilter::Phonograph(Box::new(f)));
            }
        }

        // Reverb
        if let Some(ref r) = config.reverb {
            let f = reverb::ReverbFilter::new(
                r.mix.unwrap_or(0.0),
                r.room_size.unwrap_or(0.5),
                r.damping.unwrap_or(0.5),
                r.width.unwrap_or(1.0),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Reverb(Box::new(f)));
            }
        }

        // Spatial
        if let Some(ref s) = config.spatial {
            let f = spatial::SpatialFilter::new(s.rate.unwrap_or(0.0), s.depth.unwrap_or(0.0));
            if f.is_enabled() {
                filters.push(ConcreteFilter::Spatial(Box::new(f)));
            }
        }

        // Timescale (separate — changes buffer length)
        let timescale = config.timescale.as_ref().and_then(|t| {
            let f = timescale::TimescaleFilter::new(
                t.speed.unwrap_or(1.0),
                t.pitch.unwrap_or(1.0),
                t.rate.unwrap_or(1.0),
            );
            if f.is_enabled() { Some(f) } else { None }
        });

        Self {
            filters,
            timescale,
            timescale_buffer: Vec::new(),
        }
    }

    /// Check if any filter is active.
    pub fn is_active(&self) -> bool {
        !self.filters.is_empty() || self.timescale.is_some()
    }

    /// Process audio samples through all active filters in-place.
    /// For timescale, output is buffered internally and fed to `fill_frame`.
    pub fn process(&mut self, samples: &mut [i16]) {
        for filter in self.filters.iter_mut() {
            filter.process(samples);
        }

        if let Some(ref mut ts) = self.timescale {
            let resampled = ts.process_resample(samples);
            self.timescale_buffer.extend_from_slice(&resampled);

            const MAX_TS_SAMPLES: usize = 1920 * 64;
            if self.timescale_buffer.len() > MAX_TS_SAMPLES {
                let excess = self.timescale_buffer.len() - MAX_TS_SAMPLES;
                self.timescale_buffer.drain(..excess);
            }
        }
    }

    /// When timescale is active, drain exactly `frame_size` samples from the
    /// internal buffer into `output`. Returns `true` if enough data was available.
    pub fn fill_frame(&mut self, output: &mut [i16]) -> bool {
        if self.timescale.is_none() {
            return false; // Not using timescale, caller should use the original buffer
        }

        if self.timescale_buffer.len() >= output.len() {
            output.copy_from_slice(&self.timescale_buffer[..output.len()]);
            self.timescale_buffer.drain(..output.len());
            true
        } else {
            false // Not enough data yet — skip this frame
        }
    }

    /// Whether timescale is active (changes the speak_loop flow).
    pub fn has_timescale(&self) -> bool {
        self.timescale.is_some()
    }

    pub fn reset(&mut self) {
        for filter in self.filters.iter_mut() {
            filter.reset();
        }
        if let Some(ref mut ts) = self.timescale {
            ts.reset();
        }
        self.timescale_buffer.clear();
    }
}
