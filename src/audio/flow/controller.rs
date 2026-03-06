//! `FlowController` — the central PCM processing hub.
//!
//! Reassembles arbitrary PCM chunks into fixed 3840-byte (960 sample) frames
//! and pipes them through the effects chain: Filters → Tape → Volume → Fade.

use flume::{Receiver, Sender};

use crate::audio::{
    buffer::PooledBuffer,
    constants::FRAME_SIZE_SAMPLES,
    effects::{
        crossfade::CrossfadeController, fade::FadeEffect, tape::TapeEffect, volume::VolumeEffect,
    },
    error::AudioError,
    filters::FilterChain,
};

pub struct FlowController {
    pub tape: TapeEffect,
    pub volume: VolumeEffect,
    pub fade: FadeEffect,
    pub crossfade: CrossfadeController,
    pub filters: Option<FilterChain>,
    pending_pcm: Vec<i16>,
    decoder_done: bool,
    pcm_rx: Receiver<PooledBuffer>,
    pcm_tx: Option<Sender<PooledBuffer>>,
}

impl FlowController {
    pub fn new(
        pcm_rx: Receiver<PooledBuffer>,
        pcm_tx: Sender<PooledBuffer>,
        sample_rate: u32,
        channels: usize,
    ) -> Self {
        Self::build(pcm_rx, Some(pcm_tx), sample_rate, channels)
    }

    pub fn for_mixer(pcm_rx: Receiver<PooledBuffer>, sample_rate: u32, channels: usize) -> Self {
        Self::build(pcm_rx, None, sample_rate, channels)
    }

    fn build(
        pcm_rx: Receiver<PooledBuffer>,
        pcm_tx: Option<Sender<PooledBuffer>>,
        sample_rate: u32,
        channels: usize,
    ) -> Self {
        Self {
            tape: TapeEffect::new(sample_rate, channels),
            volume: VolumeEffect::new(1.0, sample_rate, channels),
            fade: FadeEffect::new(1.0, channels),
            crossfade: CrossfadeController::new(sample_rate, channels),
            filters: None,
            pending_pcm: Vec::with_capacity(FRAME_SIZE_SAMPLES * 2),
            decoder_done: false,
            pcm_rx,
            pcm_tx,
        }
    }

    pub fn run(&mut self) {
        while let Ok(pooled) = self.pcm_rx.recv() {
            self.pending_pcm.extend_from_slice(&pooled);

            while self.pending_pcm.len() >= FRAME_SIZE_SAMPLES {
                let mut frame: PooledBuffer = Vec::with_capacity(FRAME_SIZE_SAMPLES);
                frame.extend(self.pending_pcm.drain(..FRAME_SIZE_SAMPLES));
                self.process_frame(&mut frame);

                if self
                    .pcm_tx
                    .as_ref()
                    .is_some_and(|tx| tx.send(frame).is_err())
                {
                    return;
                }
            }
        }
    }

    /// Pull-based variant for use inside the `Mixer` tick.
    ///
    /// Drains the channel only until `pending_pcm` has one full frame's worth
    /// of data — this preserves backpressure so the decoder runs at real-time
    /// pace rather than buffering the entire file into memory.
    ///
    /// Returns:
    /// - `Ok(Some(frame))` — a processed 960-sample (1920 i16) frame is ready
    /// - `Ok(None)`        — not enough data yet; call again next tick
    /// - `Err(AudioError::DecoderFinished)` — decoder finished and no full frame remains
    pub fn try_pop_frame(&mut self) -> Result<Option<PooledBuffer>, AudioError> {
        if !self.decoder_done {
            while self.pending_pcm.len() < FRAME_SIZE_SAMPLES {
                match self.pcm_rx.try_recv() {
                    Ok(chunk) if chunk.is_empty() => {
                        self.pending_pcm.clear();
                        self.decoder_done = false;
                    }
                    Ok(chunk) => self.pending_pcm.extend_from_slice(&chunk),
                    Err(flume::TryRecvError::Empty) => break,
                    Err(flume::TryRecvError::Disconnected) => {
                        self.decoder_done = true;
                        break;
                    }
                }
            }
        }

        if self.pending_pcm.len() >= FRAME_SIZE_SAMPLES {
            let mut frame: PooledBuffer = Vec::with_capacity(FRAME_SIZE_SAMPLES);
            frame.extend(self.pending_pcm.drain(..FRAME_SIZE_SAMPLES));
            self.process_frame(&mut frame);
            Ok(Some(frame))
        } else if self.decoder_done {
            Err(AudioError::DecoderFinished)
        } else {
            Ok(None)
        }
    }

    pub fn process_frame(&mut self, frame: &mut [i16]) {
        if let Some(filters) = &mut self.filters {
            filters.process(frame);
        }

        self.tape.process(frame);
        self.volume.process(frame);
        self.fade.process(frame);

        self.crossfade.fill_buffer();
        if self.crossfade.is_active() {
            self.crossfade.process(frame);
        }
    }
}
