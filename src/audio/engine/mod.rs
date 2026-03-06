pub mod encoder;
pub mod passthrough;
pub mod transcode;

pub use encoder::Encoder;
pub use passthrough::PassthroughEngine;
pub use transcode::TranscodeEngine;

use crate::audio::buffer::PooledBuffer;

pub trait Engine: Send {
    fn push_pcm(&mut self, pcm: PooledBuffer) -> bool;

    fn push_opus(&mut self, _packet: std::sync::Arc<Vec<u8>>) -> bool {
        true
    }
}

pub type BoxedEngine = Box<dyn Engine>;
