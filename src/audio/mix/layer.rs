use flume::Receiver;

use crate::audio::{RingBuffer, buffer::PooledBuffer, constants::LAYER_BUFFER_SIZE};

pub struct MixLayer {
    pub id: String,
    pub rx: Receiver<PooledBuffer>,
    pub ring_buffer: RingBuffer,
    pub volume: f32,
    pub finished: bool,
}

impl MixLayer {
    pub fn new(id: String, rx: Receiver<PooledBuffer>, volume: f32) -> Self {
        Self {
            id,
            rx,
            ring_buffer: RingBuffer::new(LAYER_BUFFER_SIZE),
            volume: volume.clamp(0.0, 1.0),
            finished: false,
        }
    }

    pub fn fill(&mut self) {
        while let Ok(pooled) = self.rx.try_recv() {
            // SAFETY: `pooled` is a valid Vec<i16> and its bytes are aligned to at
            // least 1 byte. Interpreting as &[u8] is always safe for any initialized
            // integer type. The slice lives only within this block.
            let bytes = unsafe {
                std::slice::from_raw_parts(pooled.as_ptr() as *const u8, pooled.len() * 2)
            };
            self.ring_buffer.write(bytes);
            crate::audio::buffer::release_buffer(pooled);
        }
        if self.rx.is_disconnected() {
            self.finished = true;
        }
    }

    pub fn is_dead(&self) -> bool {
        self.finished && self.ring_buffer.is_empty()
    }

    pub fn accumulate(&mut self, acc: &mut [i32]) {
        let byte_count = acc.len() * 2;
        if let Some(bytes) = self.ring_buffer.read(byte_count) {
            // SAFETY: `bytes` came from RingBuffer::read() which stores exactly the
            // bytes written by fill() — valid i16 pairs, correctly aligned (RingBuffer
            // uses Vec<u8> so alignment is 1, but i16 from_raw_parts requires only
            // that bytes.len() is even and the pointer is valid, which both hold).
            let samples = unsafe {
                std::slice::from_raw_parts(bytes.as_ptr() as *const i16, bytes.len() / 2)
            };
            for (acc_val, &s) in acc.iter_mut().zip(samples.iter()) {
                *acc_val += (s as f32 * self.volume).round() as i32;
            }
        }
    }
}
