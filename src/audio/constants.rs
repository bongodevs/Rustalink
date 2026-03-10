// ── Sample / PCM ─────────────────────────────────────────────────────────────

pub const TARGET_SAMPLE_RATE: u32 = 48_000;
pub const SAMPLE_RATE_F64: f64 = 48_000.0;
pub const FRAME_SIZE_SAMPLES: usize = 960 * 2;
pub const MIXER_CHANNELS: usize = 2;
pub const OPUS_SAMPLE_RATE: u64 = 48_000;

// ── i16 PCM clip boundaries ──────────────────────────────────────────────────

pub const INT16_MAX_F: f32 = 32_767.0;
pub const INT16_MIN_F: f32 = -32_768.0;
pub const INT16_MAX_F64: f64 = 32_768.0;
pub const INV_INT16: f64 = 1.0 / INT16_MAX_F64;

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
pub const MAX_CONCURRENT_FETCHES: usize = 2;
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

pub const HALF_PI: f32 = std::f32::consts::PI / 2.0;

// ── Route Planner ────────────────────────────────────────────────────────────

pub const ROUTE_PLANNER_FAIL_EXPIRE_MS: u64 = 7 * 24 * 60 * 60 * 1_000;
