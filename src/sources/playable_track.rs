use std::sync::Arc;

use async_trait::async_trait;
use flume::{Receiver, Sender};

use crate::{
    audio::{
        AudioFrame,
        processor::{AudioProcessor, DecoderCommand},
    },
    common::AudioFormat,
    config::player::PlayerConfig,
    sources::SourcePlugin,
};

pub type DecoderOutput = (
    Receiver<AudioFrame>,
    Sender<DecoderCommand>,
    Receiver<String>,
);

pub type BoxedTrack = Arc<dyn PlayableTrack>;
pub type BoxedSource = Box<dyn SourcePlugin>;

pub struct ResolvedTrack {
    pub reader: Box<dyn symphonia::core::io::MediaSource>,
    pub hint: Option<AudioFormat>,
}

impl ResolvedTrack {
    pub fn new(
        reader: Box<dyn symphonia::core::io::MediaSource>,
        hint: Option<AudioFormat>,
    ) -> Self {
        Self { reader, hint }
    }

    pub fn without_hint(reader: Box<dyn symphonia::core::io::MediaSource>) -> Self {
        Self { reader, hint: None }
    }
}

#[async_trait]
pub trait PlayableTrack: Send + Sync + 'static {
    async fn resolve(&self) -> Result<ResolvedTrack, String>;

    fn supports_seek(&self) -> bool {
        false
    }

    fn start_decoding(self: Arc<Self>, config: PlayerConfig) -> DecoderOutput {
        let (tx, rx) = flume::bounded::<AudioFrame>((config.buffer_duration_ms / 20) as usize);
        let (cmd_tx, cmd_rx) = flume::unbounded::<DecoderCommand>();
        let (err_tx, err_rx) = flume::bounded::<String>(1);

        let supports_seek = self.supports_seek();

        tokio::spawn(async move {
            let ResolvedTrack { reader, hint } = match self.resolve().await {
                Ok(r) => r,
                Err(e) => {
                    let _ = err_tx.send(e);
                    return;
                }
            };

            let err_tx_clone = err_tx.clone();
            tokio::task::spawn_blocking(move || {
                match AudioProcessor::new(
                    reader,
                    hint,
                    tx,
                    cmd_rx,
                    Some(err_tx_clone.clone()),
                    config,
                ) {
                    Ok(mut processor) => {
                        let result = if supports_seek {
                            processor.run_with_seek()
                        } else {
                            processor.run()
                        };
                        if let Err(e) = result {
                            let _ = err_tx_clone.send(format!(
                                "Playback failed: {e} (hint={hint:?}, seek={supports_seek})"
                            ));
                        }
                    }
                    Err(e) => {
                        let _ = err_tx_clone.send(format!(
                            "Failed to initialize audio processor: {e} (hint={hint:?})"
                        ));
                    }
                }
            });
        });

        (rx, cmd_tx, err_rx)
    }
}
