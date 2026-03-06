use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use crate::audio::constants::{MAX_BUCKET_ENTRIES, MAX_POOL_BYTES, POOL_IDLE_CLEAR_SECS};

struct PoolInner {
    buckets: HashMap<usize, Vec<Vec<u8>>>,
    total_bytes: usize,
    last_activity: Instant,
    last_cleanup: Instant,
}

impl PoolInner {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            buckets: HashMap::new(),
            total_bytes: 0,
            last_activity: now,
            last_cleanup: now,
        }
    }

    fn aligned_size(size: usize) -> usize {
        size.max(1024).next_power_of_two()
    }

    fn acquire(&mut self, size: usize) -> Vec<u8> {
        self.last_activity = Instant::now();
        let aligned = Self::aligned_size(size);

        if let Some(buf) = self
            .buckets
            .get_mut(&aligned)
            .and_then(|bucket| bucket.pop())
        {
            self.total_bytes -= aligned;
            return buf;
        }
        Vec::with_capacity(aligned)
    }

    fn release(&mut self, mut buf: Vec<u8>) {
        self.last_activity = Instant::now();
        let size = buf.capacity();

        // Only pool buffers in the 1 KB – 10 MB range.
        if !(1024..=10 * 1024 * 1024).contains(&size) {
            return;
        }
        if self.total_bytes + size > MAX_POOL_BYTES {
            return;
        }

        let bucket = self.buckets.entry(size).or_default();
        if bucket.len() >= MAX_BUCKET_ENTRIES {
            return;
        }

        buf.clear();
        self.total_bytes += size;
        bucket.push(buf);
    }

    fn cleanup(&mut self) {
        if self.total_bytes == 0 {
            return;
        }

        // Rate-limit cleanup checks to every 30 seconds.
        if self.last_cleanup.elapsed() < Duration::from_secs(30) {
            return;
        }
        self.last_cleanup = Instant::now();

        let is_idle = self.last_activity.elapsed() >= Duration::from_secs(POOL_IDLE_CLEAR_SECS);
        let is_over_limit = self.total_bytes > MAX_POOL_BYTES;

        if is_idle || is_over_limit {
            self.buckets.clear();
            self.total_bytes = 0;
        }
    }
}

pub struct BufferPool {
    inner: Mutex<PoolInner>,
}

impl BufferPool {
    fn new() -> Self {
        Self {
            inner: Mutex::new(PoolInner::new()),
        }
    }

    pub fn acquire(&self, size: usize) -> Vec<u8> {
        let mut g = self.inner.lock().unwrap();
        g.cleanup();
        g.acquire(size)
    }

    pub fn release(&self, buf: Vec<u8>) {
        let mut g = self.inner.lock().unwrap();
        g.release(buf);
    }

    pub fn stats(&self) -> PoolStats {
        let g = self.inner.lock().unwrap();
        PoolStats {
            total_bytes: g.total_bytes,
            buckets: g.buckets.len(),
            entries: g.buckets.values().map(|b| b.len()).sum(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub total_bytes: usize,
    pub buckets: usize,
    pub entries: usize,
}

static GLOBAL_BYTE_POOL: OnceLock<Arc<BufferPool>> = OnceLock::new();

pub fn get_byte_pool() -> Arc<BufferPool> {
    GLOBAL_BYTE_POOL
        .get_or_init(|| Arc::new(BufferPool::new()))
        .clone()
}
