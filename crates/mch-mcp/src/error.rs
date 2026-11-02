//! Error taxonomy for the MCP client.
//!
//! Forged from Goose `crates/goose/src/agents/extension.rs:40`. The point of the
//! split is that a startup failure, a server-level error, and a transport death
//! are three different things and must not collapse into one opaque string.

use thiserror::Error;

/// The child process exited (or its pipe closed) before the handshake completed.
/// Carries the buffered stderr, which is the difference between "server failed"
/// and "server failed because `uvx` is not on PATH".
#[derive(Debug, Error)]
#[error("process quit before initialization: stderr = {stderr}")]
pub struct ProcessExit {
    pub stderr: String,
    pub detail: String,
}

impl ProcessExit {
    pub fn new(stderr: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            stderr: stderr.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum McpError {
    /// The transport is gone (pipe closed, socket dropped).
    #[error("transport closed")]
    TransportClosed,

    /// The request exceeded its budget. A `notifications/cancelled` was sent
    /// upstream before this was returned.
    #[error("request timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// The caller cancelled the request.
    #[error("operation cancelled")]
    Cancelled,

    /// The server answered with the wrong shape.
    #[error("unexpected response from server")]
    UnexpectedResponse,

    /// The handshake could not be completed.
    #[error("failed to initialize MCP client: {0}")]
    Initialize(String),

    /// Malformed or unsafe configuration.
    #[error("invalid configuration: {0}")]
    ConfigError(String),

    /// The child died at startup.
    #[error("{0}")]
    ProcessExit(#[from] ProcessExit),

    /// Server-side JSON-RPC error object.
    #[error("server error {code}: {message}")]
    Server { code: i64, message: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type McpResult<T> = Result<T, McpError>;
