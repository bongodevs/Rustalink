use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::common::types::AnyError;

#[derive(Serialize, Deserialize, Debug)]
pub struct VoiceGatewayMessage {
    pub op: u8,
    pub d: Value,
}

/// What the outer reconnect loop should do after a session ends.
pub enum SessionOutcome {
    /// Op 7 resume is viable — the UDP/SSRC state is still valid on Discord's end.
    Reconnect,
    /// Session is stale; start fresh with Op 0 Identify.
    Identify,
    /// Close code is fatal or retry budget exhausted — stop entirely.
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseAction {
    UnknownOpcode,
    InvalidPayload,
    NotAuthenticated,
    AuthenticationFailed,
    AlreadyAuthenticated,
    InvalidSession,
    SessionTimeout,
    ServerNotFound,
    UnknownProtocol,
    Disconnected,
    VoiceServerCrash,
    UnknownEncryptionMode,
    DaveProtocolRequired,
    BadRequest,
    RateLimited,
    CallTerminated,
}

const CLOSE_CODE_TABLE: &[(u16, CloseAction)] = &[
    (4001, CloseAction::UnknownOpcode),
    (4002, CloseAction::InvalidPayload),
    (4003, CloseAction::NotAuthenticated),
    (4004, CloseAction::AuthenticationFailed),
    (4005, CloseAction::AlreadyAuthenticated),
    (4006, CloseAction::InvalidSession),
    (4009, CloseAction::SessionTimeout),
    (4011, CloseAction::ServerNotFound),
    (4012, CloseAction::UnknownProtocol),
    (4013, CloseAction::Disconnected),
    (4014, CloseAction::VoiceServerCrash),
    (4015, CloseAction::UnknownEncryptionMode),
    (4016, CloseAction::DaveProtocolRequired),
    (4020, CloseAction::BadRequest),
    (4021, CloseAction::RateLimited),
    (4022, CloseAction::CallTerminated),
];

/// Single authoritative entry point: given a raw WS close code, return the
/// `SessionOutcome` the reconnect loop should act on.
pub fn classify_close(code: u16) -> SessionOutcome {
    let action = CLOSE_CODE_TABLE
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, a)| *a)
        .unwrap_or(CloseAction::UnknownOpcode);

    match action {
        CloseAction::UnknownOpcode => SessionOutcome::Reconnect,
        CloseAction::InvalidPayload => SessionOutcome::Identify,
        CloseAction::NotAuthenticated => SessionOutcome::Shutdown,
        CloseAction::AuthenticationFailed => SessionOutcome::Shutdown,
        CloseAction::AlreadyAuthenticated => SessionOutcome::Shutdown,
        CloseAction::InvalidSession => SessionOutcome::Identify,
        CloseAction::SessionTimeout => SessionOutcome::Identify,
        CloseAction::ServerNotFound => SessionOutcome::Shutdown,
        CloseAction::UnknownProtocol => SessionOutcome::Shutdown,
        CloseAction::Disconnected => SessionOutcome::Shutdown,
        CloseAction::VoiceServerCrash => SessionOutcome::Shutdown,
        CloseAction::UnknownEncryptionMode => SessionOutcome::Shutdown,
        CloseAction::DaveProtocolRequired => SessionOutcome::Shutdown,
        CloseAction::BadRequest => SessionOutcome::Shutdown,
        CloseAction::RateLimited => SessionOutcome::Shutdown,
        CloseAction::CallTerminated => SessionOutcome::Shutdown,
    }
}

pub fn is_fatal_close(code: u16) -> bool {
    matches!(classify_close(code), SessionOutcome::Shutdown)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceOp {
    Ready = 2,
    Speaking = 5,
    SessionDescription = 4,
    HeartbeatAck = 6,
    Hello = 8,
    Resumed = 9,
    UserConnect = 11,
    UserDisconnect = 13,
    DavePrepareTransition = 21,
    DaveExecuteTransition = 22,
    DavePrepareEpoch = 24,
}

impl VoiceOp {
    pub fn from_raw(op: u8) -> Option<Self> {
        Some(match op {
            2 => Self::Ready,
            5 => Self::Speaking,
            4 => Self::SessionDescription,
            6 => Self::HeartbeatAck,
            8 => Self::Hello,
            9 => Self::Resumed,
            11 => Self::UserConnect,
            13 => Self::UserDisconnect,
            21 => Self::DavePrepareTransition,
            22 => Self::DaveExecuteTransition,
            24 => Self::DavePrepareEpoch,
            _ => return None,
        })
    }
}

#[inline]
pub fn map_boxed_err<E: std::fmt::Display>(e: E) -> AnyError {
    Box::new(std::io::Error::other(e.to_string()))
}
