use flume::Sender;

use super::Engine;
use crate::audio::AudioFrame;

pub struct StandardEngine {
    frame_tx: Sender<AudioFrame>,
}

impl StandardEngine {
    pub fn new(frame_tx: Sender<AudioFrame>) -> Self {
        Self { frame_tx }
    }
}

impl Engine for StandardEngine {
    fn push(&mut self, frame: AudioFrame) -> bool {
        self.frame_tx.send(frame).is_ok()
    }
}
