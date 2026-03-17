pub mod crossfade;
pub mod fade;
pub mod tape;
pub mod volume;

use std::sync::atomic::{AtomicU8, AtomicU64};

use crate::audio::buffer::PooledBuffer;

/// Context passed to [`TransitionEffect::process`] on each frame.
pub struct ProcessContext<'a> {
    pub mix_buf: &'a mut [i32],
    pub i: &'a mut usize,
    pub out_len: usize,
    pub vol: f32,
    pub stash: &'a mut Vec<i16>,
    pub rx: &'a flume::Receiver<PooledBuffer>,
    pub state_atomic: &'a AtomicU8,
    pub position_atomic: &'a AtomicU64,
}

/// Interface for transition effects.
pub trait TransitionEffect: Send {
    fn process(&mut self, ctx: ProcessContext<'_>) -> bool;
}
