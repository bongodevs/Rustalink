// ── Sample / PCM ─────────────────────────────────────────────────────────────

pub const TARGET_SAMPLE_RATE: u32 = 48_000;
pub const SAMPLE_RATE_F64: f64 = 48_000.0;
pub const FRAME_SIZE_SAMPLES: usize = 960 * 2;
pub const MIXER_CHANNELS: usize = 2;

// ── i16 PCM clip boundaries ──────────────────────────────────────────────────

pub const INT16_MAX_F: f32 = 32_767.0;
pub const INT16_MIN_F: f32 = -32_768.0;
/// Normalization divisor: i16 range mapped to [-1.0, 1.0].
pub const INT16_NORM_F64: f64 = 32_768.0;
pub const INV_INT16: f64 = 1.0 / INT16_NORM_F64;

// ── Codec ────────────────────────────────────────────────────────────────────

pub const MAX_OPUS_FRAME_SIZE: usize = 5_760;

// ── Buffer pool (byte pool) ──────────────────────────────────────────────────

pub const MAX_POOL_BYTES: usize = 2 * 1_024 * 1_024;
pub const MAX_BUCKET_ENTRIES: usize = 8;
pub const POOL_IDLE_CLEAR_SECS: u64 = 180;

// ── Audio mixer layers ───────────────────────────────────────────────────────

pub const MAX_LAYERS: usize = 5;
pub const LAYER_BUFFER_SIZE: usize = 1_024 * 1_024;

// ── Segmented remote reader ──────────────────────────────────────────────────

pub const CHUNK_SIZE: usize = 256 * 1_024;
pub const PREFETCH_CHUNKS: usize = 4;
pub const MAX_CONCURRENT_FETCHES: usize = 4;
pub const HTTP_CLIENT_TIMEOUT_SECS: u64 = 15;
pub const MAX_FETCH_RETRIES: u32 = 5;
pub const WORKER_IDLE_MS: u64 = 50;
pub const FETCH_WAIT_MS: u64 = 250;
pub const PROBE_TIMEOUT_SECS: u64 = 10;

// ── HttpSource ───────────────────────────────────────────────────────────────

pub const HTTP_PREFETCH_BUFFER_SIZE: usize = 2 * 1_024 * 1_024;
pub const MAX_HTTP_BUF_BYTES: usize = 8 * 1_024 * 1_024;
pub const HTTP_INITIAL_BUF_CAPACITY: usize = 256 * 1_024;
pub const HTTP_SOCKET_SKIP_LIMIT: u64 = 1_000_000;
pub const HTTP_FETCH_CHUNK_LIMIT: u64 = 2 * 1_024 * 1_024;

// ── Effects ──────────────────────────────────────────────────────────────────

pub use std::f32::consts::FRAC_PI_2 as HALF_PI;

/// Sample rate used for Opus ms↔sample conversions (48 kHz as u64).
pub const OPUS_SAMPLE_RATE: u64 = TARGET_SAMPLE_RATE as u64;

// ── Route Planner ────────────────────────────────────────────────────────────

pub const ROUTE_PLANNER_FAIL_EXPIRE_MS: u64 = 7 * 24 * 60 * 60 * 1_000;

// ── Mixer/Processor buffer sizes ─────────────────────────────────────────────

/// Default mixer buffer size in samples (40ms at 48kHz = 1920 samples)
pub const MIX_BUFFER_SIZE: usize = 1920;

/// Controller buffer size (2x frame size for lookahead)
pub const CONTROLLER_BUFFER_SIZE: usize = FRAME_SIZE_SAMPLES * 2;

// ── Phonograph filter delay lines ────────────────────────────────────────────

/// Maximum delay in milliseconds for phonograph effect
pub const PHONOGRAPH_MAX_DELAY_MS: f32 = 60.0;

/// Phonograph room delay line sizes (in samples at 48kHz)
pub const PHONOGRAPH_R1_SIZE: usize = 148 * 48;
pub const PHONOGRAPH_R2_SIZE: usize = 115 * 48;
pub const PHONOGRAPH_R3_SIZE: usize = 63 * 48;

// ── Karaoke filter constants ─────────────────────────────────────────────────

/// Karaoke filter scale factor for i16 conversion
pub const KARAOKE_SCALE: f64 = 32768.0;
/// Inverse of KARAOKE_SCALE
pub const KARAOKE_INV: f64 = 1.0 / KARAOKE_SCALE;
/// Maximum output gain for karaoke filter to prevent clipping
pub const KARAOKE_MAX_OUTPUT_GAIN: f64 = 0.98;
