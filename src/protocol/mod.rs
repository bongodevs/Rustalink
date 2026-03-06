use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodecError {
    #[error("Empty input")]
    EmptyInput,
    #[error("Corrupt buffer: {0}")]
    CorruptBuffer(String),
    #[error("Truncated buffer: {0}")]
    TruncatedBuffer(String),
    #[error("Unknown track version: {0}")]
    UnknownVersion(u8),
    #[error("Base64 error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

pub mod codec;
pub mod events;
pub mod info;
pub mod models;
pub mod opcodes;
pub mod routeplanner;
pub mod session;
pub mod stats;
pub mod tracks;

pub use codec::*;
pub use events::*;
pub use info::*;
pub use models::*;
pub use routeplanner::*;
pub use session::*;
pub use stats::*;
pub use tracks::*;
