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
            // SAFETY: i16 slice reinterpreted as u8 bytes for RingBuffer storage.
            let bytes = unsafe {
                std::slice::from_raw_parts(pooled.as_ptr() as *const u8, pooled.len() * 2)
            };
            self.ring_buffer.write(bytes);
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
            // SAFETY: u8 bytes reinterpreted as i16 samples.
            let samples = unsafe {
                std::slice::from_raw_parts(bytes.as_ptr() as *const i16, bytes.len() / 2)
            };
            for (acc_val, &s) in acc.iter_mut().zip(samples.iter()) {
                *acc_val += (s as f32 * self.volume).round() as i32;
            }
        }
    }
}
