use audiopus::{Channels, MutSignals, SampleRate, coder::Decoder as OpusDecoder, packet::Packet};
use symphonia::core::{
    audio::{AsAudioBufferRef, AudioBuffer, AudioBufferRef, Layout, Signal, SignalSpec},
    codecs::{
        CODEC_TYPE_OPUS, CodecDescriptor, CodecParameters, Decoder, DecoderOptions, FinalizeResult,
    },
    errors::{Error, Result},
    formats::Packet as SymphPacket,
    units::Duration,
};

use crate::audio::constants::MAX_OPUS_FRAME_SIZE;

pub struct OpusCodecDecoder {
    params: CodecParameters,
    channels: usize,
    decoder: OpusDecoder,
    buf: AudioBuffer<i16>,
    pcm: Vec<i16>,
}

unsafe impl Sync for OpusCodecDecoder {}

#[inline]
fn opus_channels(n: usize) -> Channels {
    if n == 1 {
        Channels::Mono
    } else {
        Channels::Stereo
    }
}

#[inline]
fn opus_layout(n: usize) -> Layout {
    if n == 1 { Layout::Mono } else { Layout::Stereo }
}

impl Decoder for OpusCodecDecoder {
    fn try_new(params: &CodecParameters, _options: &DecoderOptions) -> Result<Self> {
        if params.codec != CODEC_TYPE_OPUS {
            return Err(Error::Unsupported("not an opus stream"));
        }

        let sample_rate = params.sample_rate.unwrap_or(48000);
        let channels = params.channels.map(|c| c.count()).unwrap_or(2).clamp(1, 2);

        let opus_rate = match sample_rate {
            8000 => SampleRate::Hz8000,
            12000 => SampleRate::Hz12000,
            16000 => SampleRate::Hz16000,
            24000 => SampleRate::Hz24000,
            _ => SampleRate::Hz48000,
        };

        let decoder = OpusDecoder::new(opus_rate, opus_channels(channels))
            .map_err(|e| Error::IoError(std::io::Error::other(e)))?;

        let spec = SignalSpec::new_with_layout(sample_rate, opus_layout(channels));
        let buf = AudioBuffer::<i16>::new(MAX_OPUS_FRAME_SIZE as Duration, spec);
        let pcm = vec![0i16; MAX_OPUS_FRAME_SIZE * channels];

        Ok(Self {
            params: params.clone(),
            channels,
            decoder,
            buf,
            pcm,
        })
    }

    fn supported_codecs() -> &'static [CodecDescriptor] {
        &[CodecDescriptor {
            codec: CODEC_TYPE_OPUS,
            short_name: "opus",
            long_name: "Opus (via audiopus)",
            inst_func: |params, opts| Ok(Box::new(OpusCodecDecoder::try_new(params, opts)?)),
        }]
    }

    fn reset(&mut self) {
        match OpusDecoder::new(SampleRate::Hz48000, opus_channels(self.channels)) {
            Ok(dec) => self.decoder = dec,
            Err(e) => tracing::warn!("opus decoder reset failed: {e}"),
        }
    }

    fn codec_params(&self) -> &CodecParameters {
        &self.params
    }

    fn decode(&mut self, packet: &SymphPacket) -> Result<AudioBufferRef<'_>> {
        let n = self
            .decoder
            .decode(
                Packet::try_from(packet.data.as_ref()).ok(),
                MutSignals::try_from(self.pcm.as_mut_slice())
                    .map_err(|e| Error::IoError(std::io::Error::other(e)))?,
                false,
            )
            .map_err(|e| Error::IoError(std::io::Error::other(e)))?;

        self.buf.clear();
        self.buf.render_reserved(Some(n));

        let ch = self.channels;
        if ch == 1 {
            let plane = self.buf.chan_mut(0);
            plane.copy_from_slice(&self.pcm[..n]);
        } else {
            let left = self.buf.chan_mut(0);
            for (i, chunk) in self.pcm[..n * 2].chunks_exact(2).enumerate() {
                left[i] = chunk[0];
            }
            let right = self.buf.chan_mut(1);
            for (i, chunk) in self.pcm[..n * 2].chunks_exact(2).enumerate() {
                right[i] = chunk[1];
            }
        }

        Ok(self.buf.as_audio_buffer_ref())
    }

    fn finalize(&mut self) -> FinalizeResult {
        FinalizeResult::default()
    }

    fn last_decoded(&self) -> AudioBufferRef<'_> {
        self.buf.as_audio_buffer_ref()
    }
}
