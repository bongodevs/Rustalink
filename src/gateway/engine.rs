use tokio::sync::Mutex;

use crate::{audio::Mixer, common::types::Shared, gateway::constants::DEFAULT_SAMPLE_RATE};

pub struct VoiceEngine {
    pub mixer: Shared<Mixer>,
}

impl VoiceEngine {
    pub fn new() -> Self {
        Self {
            mixer: Shared::new(Mutex::new(Mixer::new(DEFAULT_SAMPLE_RATE))),
        }
    }
}

impl Default for VoiceEngine {
    fn default() -> Self {
        Self::new()
    }
}
