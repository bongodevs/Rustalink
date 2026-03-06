use flume::Sender;

use super::Engine;
use crate::audio::buffer::PooledBuffer;

pub struct TranscodeEngine {
    pcm_tx: Sender<PooledBuffer>,
}

impl TranscodeEngine {
    pub fn new(pcm_tx: Sender<PooledBuffer>) -> Self {
        Self { pcm_tx }
    }
}

impl Engine for TranscodeEngine {
    fn push_pcm(&mut self, pcm: PooledBuffer) -> bool {
        self.pcm_tx.send(pcm).is_ok()
    }
}
