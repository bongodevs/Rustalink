use std::sync::Arc;

use flume::Sender;

use super::Engine;
use crate::audio::buffer::PooledBuffer;

pub struct PassthroughEngine {
    opus_tx: Sender<Arc<Vec<u8>>>,
}

impl PassthroughEngine {
    pub fn new(opus_tx: Sender<Arc<Vec<u8>>>) -> Self {
        Self { opus_tx }
    }
}

impl Engine for PassthroughEngine {
    fn push_pcm(&mut self, _pcm: PooledBuffer) -> bool {
        true
    }

    fn push_opus(&mut self, packet: Arc<Vec<u8>>) -> bool {
        self.opus_tx.send(packet).is_ok()
    }
}
