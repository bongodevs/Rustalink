use serde::{Deserialize, Serialize};

use crate::common::utils::now_ms;

/// Exception severity levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Common,
    Suspicious,
    Fault,
}

/// Rustalink v4 JSON error response format.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RustalinkError {
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
    /// HTTP status code.
    pub status: u16,
    /// HTTP status reason phrase (e.g. "Bad Request").
    pub error: String,
    /// Human-readable error message.
    pub message: String,
    /// The request path that caused the error.
    pub path: String,
    /// Stack trace (only in non-production).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<String>,
}

impl RustalinkError {
    /// Creates a 400 Bad Request error.
    pub fn bad_request(message: impl Into<String>, path: impl Into<String>) -> Self {
        Self::new(400, "Bad Request", message, path)
    }

    /// Creates a 404 Not Found error.
    pub fn not_found(message: impl Into<String>, path: impl Into<String>) -> Self {
        Self::new(404, "Not Found", message, path)
    }

    /// Creates a generic error response.
    pub fn new(
        status: u16,
        error: impl Into<String>,
        message: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: now_ms(),
            status,
            error: error.into(),
            message: message.into(),
            path: path.into(),
            trace: None,
        }
    }
}
