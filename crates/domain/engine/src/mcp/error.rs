//! The crate's error type and the helper that renders an error with its causes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use serde_json::Value;

/// A boxed error from code outside this crate, such as a
/// [`crate::mcp::HeaderSource`].
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Everything that can go wrong talking to a server. No variant carries a
/// header value.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("server error {code}: {message}")]
    Rpc {
        code: i64,
        message: String,
        // JSON: the JSON-RPC error object's `data` member, any shape the server chose.
        data: Option<Box<Value>>,
    },
    #[error("request timed out after {}s", .0.as_secs())]
    Timeout(Duration),
    #[error("request was cancelled")]
    Cancelled,
    #[error("connection to the server is closed")]
    Closed,
    #[error("the server session expired")]
    SessionExpired,
    #[error("the server requires authorization")]
    Unauthorized,
    #[error("the server answered HTTP {status}")]
    Status { status: u16 },
    #[error("the server answered with unexpected content type {content_type:?}")]
    ContentType { content_type: String },
    #[error("HTTP request to the server failed")]
    Http(#[source] reqwest::Error),
    #[error("invalid JSON in a server message")]
    Json(#[source] serde_json::Error),
    #[error("server pipe I/O failed")]
    Io(#[source] std::io::Error),
    #[error("failed to start {command}")]
    Spawn {
        command: String,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid server URL")]
    Url(#[source] url::ParseError),
    #[error("invalid header {name}")]
    Header { name: String },
    #[error("the header source failed")]
    HeaderSource(#[source] BoxError),
    #[error("unsupported protocol version {version:?}")]
    ProtocolVersion { version: String },
    #[error("protocol violation: {0}")]
    Protocol(&'static str),
    #[error("connecting timed out after {}s", .0.as_secs())]
    ConnectTimeout(Duration),
    #[error("unknown tool {tool}")]
    UnknownTool { tool: String },
    #[error("unknown server {server}")]
    UnknownServer { server: String },
    #[error("server {server} is not connected")]
    NotConnected { server: String },
    #[error("no tokio runtime is running")]
    NoRuntime(#[source] tokio::runtime::TryCurrentError),
}

impl McpError {
    // Why: reqwest puts the request URL in its Display, and some servers carry
    // credentials in the query string.
    pub(crate) fn http(error: reqwest::Error) -> Self {
        Self::Http(error.without_url())
    }
}

pub(crate) fn describe(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}
