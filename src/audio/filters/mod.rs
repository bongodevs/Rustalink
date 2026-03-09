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

pub fn validate_filters(filters: &Filters, config: &FiltersConfig) -> Vec<&'static str> {
    let mut invalid = Vec::new();
    let c = config;
    let f = filters;

    if f.volume.is_some() && !c.volume {
        invalid.push("volume");
    }
    if f.equalizer.is_some() && !c.equalizer {
        invalid.push("equalizer");
    }
    if f.karaoke.is_some() && !c.karaoke {
        invalid.push("karaoke");
    }
    if f.timescale.is_some() && !c.timescale {
        invalid.push("timescale");
    }
    if f.tremolo.is_some() && !c.tremolo {
        invalid.push("tremolo");
    }
    if f.vibrato.is_some() && !c.vibrato {
        invalid.push("vibrato");
    }
    if f.distortion.is_some() && !c.distortion {
        invalid.push("distortion");
    }
    if f.rotation.is_some() && !c.rotation {
        invalid.push("rotation");
    }
    if f.channel_mix.is_some() && !c.channel_mix {
        invalid.push("channelMix");
    }
    if f.low_pass.is_some() && !c.low_pass {
        invalid.push("lowPass");
    }
    if f.echo.is_some() && !c.echo {
        invalid.push("echo");
    }
    if f.high_pass.is_some() && !c.high_pass {
        invalid.push("highPass");
    }
    if f.normalization.is_some() && !c.normalization {
        invalid.push("normalization");
    }
    if f.chorus.is_some() && !c.chorus {
        invalid.push("chorus");
    }
    if f.compressor.is_some() && !c.compressor {
        invalid.push("compressor");
    }
    if f.flanger.is_some() && !c.flanger {
        invalid.push("flanger");
    }
    if f.phaser.is_some() && !c.phaser {
        invalid.push("phaser");
    }
    if f.phonograph.is_some() && !c.phonograph {
        invalid.push("phonograph");
    }
    if f.reverb.is_some() && !c.reverb {
        invalid.push("reverb");
    }
    if f.spatial.is_some() && !c.spatial {
        invalid.push("spatial");
    }

    invalid
}

pub trait AudioFilter: Send {
    fn process(&mut self, samples: &mut [i16]);
    fn is_enabled(&self) -> bool;
    fn reset(&mut self);
}

pub enum ConcreteFilter {
    Volume(volume::VolumeFilter),
    Equalizer(Box<equalizer::EqualizerFilter>),
    Karaoke(karaoke::KaraokeFilter),
    Tremolo(tremolo::TremoloFilter),
    Vibrato(vibrato::VibratoFilter),
    Rotation(rotation::RotationFilter),
    Distortion(distortion::DistortionFilter),
    ChannelMix(channel_mix::ChannelMixFilter),
    LowPass(low_pass::LowPassFilter),
    Echo(echo::EchoFilter),
    HighPass(high_pass::HighPassFilter),
    Normalization(normalization::NormalizationFilter),
    Chorus(chorus::ChorusFilter),
    Compressor(compressor::CompressorFilter),
    Flanger(flanger::FlangerFilter),
    Phaser(phaser::PhaserFilter),
    Phonograph(Box<phonograph::PhonographFilter>),
    Reverb(reverb::ReverbFilter),
    Spatial(spatial::SpatialFilter),
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

pub struct FilterChain {
    filters: Vec<ConcreteFilter>,
    timescale: Option<timescale::TimescaleFilter>,
    timescale_buffer: Vec<i16>,
}

impl FilterChain {
    pub fn from_config(config: &Filters) -> Self {
        let mut filters = Vec::new();

        if let Some(vol) = config.volume {
            let f = volume::VolumeFilter::new(vol);
            if f.is_enabled() {
                filters.push(ConcreteFilter::Volume(f));
            }
        }

        if let Some(ref bands) = config.equalizer {
            let band_tuples: Vec<(u8, f32)> =
                bands.iter().map(|b: &EqBand| (b.band, b.gain)).collect();
            let f = equalizer::EqualizerFilter::new(&band_tuples);
            if f.is_enabled() {
                filters.push(ConcreteFilter::Equalizer(Box::new(f)));
            }
        }

        if let Some(ref k) = config.karaoke {
            let f = karaoke::KaraokeFilter::new(
                k.level.unwrap_or(1.0),
                k.mono_level.unwrap_or(1.0),
                k.filter_band.unwrap_or(220.0),
                k.filter_width.unwrap_or(100.0),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Karaoke(f));
            }
        }

        if let Some(ref t) = config.tremolo {
            let f = tremolo::TremoloFilter::new(t.frequency.unwrap_or(2.0), t.depth.unwrap_or(0.5));
            if f.is_enabled() {
                filters.push(ConcreteFilter::Tremolo(f));
            }
        }

        if let Some(ref v) = config.vibrato {
            let f = vibrato::VibratoFilter::new(v.frequency.unwrap_or(2.0), v.depth.unwrap_or(0.5));
            if f.is_enabled() {
                filters.push(ConcreteFilter::Vibrato(f));
            }
        }

        if let Some(ref r) = config.rotation {
            let f = rotation::RotationFilter::new(r.rotation_hz.unwrap_or(0.0));
            if f.is_enabled() {
                filters.push(ConcreteFilter::Rotation(f));
            }
        }

        if let Some(ref d) = config.distortion {
            let f = distortion::DistortionFilter::new(
                d.sin_offset.unwrap_or(0.0),
                d.sin_scale.unwrap_or(1.0),
                d.cos_offset.unwrap_or(0.0),
                d.cos_scale.unwrap_or(1.0),
                d.tan_offset.unwrap_or(0.0),
                d.tan_scale.unwrap_or(1.0),
                d.offset.unwrap_or(0.0),
                d.scale.unwrap_or(1.0),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Distortion(f));
            }
        }

        if let Some(ref cm) = config.channel_mix {
            let f = channel_mix::ChannelMixFilter::new(
                cm.left_to_left.unwrap_or(1.0),
                cm.left_to_right.unwrap_or(0.0),
                cm.right_to_left.unwrap_or(0.0),
                cm.right_to_right.unwrap_or(1.0),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::ChannelMix(f));
            }
        }

        if let Some(ref lp) = config.low_pass {
            let f = low_pass::LowPassFilter::new(lp.smoothing.unwrap_or(20.0));
            if f.is_enabled() {
                filters.push(ConcreteFilter::LowPass(f));
            }
        }

        if let Some(ref e) = config.echo {
            let f = echo::EchoFilter::new(e.echo_length.unwrap_or(1.0), e.decay.unwrap_or(0.5));
            if f.is_enabled() {
                filters.push(ConcreteFilter::Echo(f));
            }
        }

        if let Some(ref hp) = config.high_pass {
            let f = high_pass::HighPassFilter::new(
                hp.cutoff_frequency.unwrap_or(200),
                hp.boost_factor.unwrap_or(1.0),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::HighPass(f));
            }
        }

        if let Some(ref n) = config.normalization {
            let f = normalization::NormalizationFilter::new(
                n.max_amplitude.unwrap_or(1.0),
                n.adaptive.unwrap_or(true),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Normalization(f));
            }
        }

        if let Some(ref c) = config.chorus {
            let f = chorus::ChorusFilter::new(
                c.rate.unwrap_or(1.5),
                c.depth.unwrap_or(1.0),
                c.delay.unwrap_or(2.0),
                c.mix.unwrap_or(0.5),
                c.feedback.unwrap_or(0.5),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Chorus(f));
            }
        }

        if let Some(ref c) = config.compressor {
            let f = compressor::CompressorFilter::new(
                c.threshold.unwrap_or(-10.0),
                c.ratio.unwrap_or(2.0),
                c.attack.unwrap_or(5.0),
                c.release.unwrap_or(50.0),
                c.makeup_gain.unwrap_or(0.0),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Compressor(f));
            }
        }

        if let Some(ref fl) = config.flanger {
            let f = flanger::FlangerFilter::new(
                fl.rate.unwrap_or(0.2),
                fl.depth.unwrap_or(1.0),
                fl.feedback.unwrap_or(0.5),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Flanger(f));
            }
        }

        if let Some(ref p) = config.phaser {
            let f = phaser::PhaserFilter::new(
                p.stages.unwrap_or(4),
                p.rate.unwrap_or(0.0),
                p.depth.unwrap_or(1.0),
                p.feedback.unwrap_or(0.0),
                p.mix.unwrap_or(0.5),
                p.min_frequency.unwrap_or(100.0),
                p.max_frequency.unwrap_or(2500.0),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Phaser(f));
            }
        }

        if let Some(ref ph) = config.phonograph {
            let f = phonograph::PhonographFilter::new(
                ph.frequency.unwrap_or(0.8),
                ph.depth.unwrap_or(0.25),
                ph.crackle.unwrap_or(0.18),
                ph.flutter.unwrap_or(0.18),
                ph.room.unwrap_or(0.22),
                ph.mic_agc.unwrap_or(0.25),
                ph.drive.unwrap_or(0.25),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Phonograph(Box::new(f)));
            }
        }

        if let Some(ref r) = config.reverb {
            let f = reverb::ReverbFilter::new(
                r.mix.unwrap_or(0.0),
                r.room_size.unwrap_or(0.5),
                r.damping.unwrap_or(0.5),
                r.width.unwrap_or(1.0),
            );
            if f.is_enabled() {
                filters.push(ConcreteFilter::Reverb(f));
            }
        }

        if let Some(ref s) = config.spatial {
            let f = spatial::SpatialFilter::new(s.rate.unwrap_or(0.0), s.depth.unwrap_or(0.0));
            if f.is_enabled() {
                filters.push(ConcreteFilter::Spatial(f));
            }
        }

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

    pub fn is_active(&self) -> bool {
        !self.filters.is_empty() || self.timescale.is_some()
    }

    pub fn process(&mut self, samples: &mut [i16]) {
        for filter in self.filters.iter_mut() {
            filter.process(samples);
        }

        if let Some(ref mut ts) = self.timescale {
            let resampled = ts.process_resample(samples);
            self.timescale_buffer.extend_from_slice(&resampled);

            const MAX_TS_SAMPLES: usize = 1920 * 1024;
            if self.timescale_buffer.len() > MAX_TS_SAMPLES {
                let excess = self.timescale_buffer.len() - MAX_TS_SAMPLES;
                let excess = excess - (excess % 2);
                if excess > 0 {
                    self.timescale_buffer.drain(..excess);
                }
            }
        }
    }

    pub fn fill_frame(&mut self, output: &mut [i16]) -> bool {
        if self.timescale.is_none() {
            return false;
        }

        if self.timescale_buffer.len() >= output.len() {
            output.copy_from_slice(&self.timescale_buffer[..output.len()]);
            self.timescale_buffer.drain(..output.len());
            true
        } else {
            false
        }
    }

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
