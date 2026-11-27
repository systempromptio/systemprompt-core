//! Errors of the language server client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use crate::lsp::framing::FrameError;
use crate::lsp::spec::ServerId;

/// Everything that can go wrong between the engine and a language server.
#[derive(Debug, thiserror::Error)]
pub enum LspError {
    #[error("no language server is configured for {}", path.display())]
    NoServer { path: PathBuf },

    #[error("no language server is running for this worktree")]
    NoRunningServer,

    #[error("the language servers for {} do not support {}", path.display(), operation)]
    Unsupported {
        operation: &'static str,
        path: PathBuf,
    },

    #[error("language server {server} is unavailable: {reason}")]
    Unavailable { server: ServerId, reason: String },

    #[error("failed to start language server {server}")]
    Spawn {
        server: ServerId,
        #[source]
        source: std::io::Error,
    },

    #[error("language server {server} did not answer {method} within {timeout_ms} ms")]
    Timeout {
        server: ServerId,
        method: String,
        timeout_ms: u128,
    },

    #[error("language server {server} rejected {method} ({code}): {message}")]
    Remote {
        server: ServerId,
        method: String,
        code: i64,
        message: String,
    },

    #[error("the connection to language server {server} is closed")]
    Closed { server: ServerId },

    #[error("failed to encode a message for language server {server}")]
    Encode {
        server: ServerId,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to read {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("malformed message frame from a language server")]
    Frame(#[from] FrameError),

    #[error("the language server manager has been shut down")]
    Stopped,
}

pub(crate) fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}
