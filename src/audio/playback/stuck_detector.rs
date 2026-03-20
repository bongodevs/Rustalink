use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::common::utils::now_nanos;

pub struct StuckDetector {
    last_frame_received_at_nanos: AtomicU64,
    threshold_ms: AtomicU64,
    stuck_event_sent: AtomicBool,
}

impl StuckDetector {
    pub fn new(threshold_ms: u64) -> Self {
        Self {
            last_frame_received_at_nanos: AtomicU64::new(now_nanos()),
            threshold_ms: AtomicU64::new(threshold_ms),
            stuck_event_sent: AtomicBool::new(false),
        }
    }

    pub fn record_frame_received(&self) {
        let now_nanos = now_nanos();
        self.last_frame_received_at_nanos
            .store(now_nanos, Ordering::Release);
    }

    pub fn reset_stuck_flag(&self) {
        self.stuck_event_sent.store(false, Ordering::Release);
    }

    pub fn check_stuck(&self) -> bool {
        if self.stuck_event_sent.load(Ordering::Acquire) {
            return false;
        }

        let now_nanos = now_nanos();
        let last_received = self.last_frame_received_at_nanos.load(Ordering::Acquire);
        let elapsed_nanos = now_nanos.saturating_sub(last_received);
        let threshold_nanos = self.threshold_ms.load(Ordering::Acquire) * 1_000_000;

        if elapsed_nanos >= threshold_nanos {
            self.stuck_event_sent.store(true, Ordering::Release);
            true
        } else {
            false
        }
    }

    pub fn threshold_ms(&self) -> u64 {
        self.threshold_ms.load(Ordering::Acquire)
    }

    pub fn set_threshold(&self, threshold_ms: u64) {
        self.threshold_ms.store(threshold_ms, Ordering::Release);
    }
}

impl Default for StuckDetector {
    fn default() -> Self {
        Self::new(10_000)
    }
}
