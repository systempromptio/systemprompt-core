//! Errors of a model call, typed so the caller can react to refusals, overflow
//! and dropped connections.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

const MAX_EXCERPT_CHARS: usize = 512;

/// A failure below HTTP semantics: connecting, timing out, or reading the body.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("could not build the HTTP client")]
    Client(#[source] reqwest::Error),
    #[error("could not connect to the model gateway")]
    Connect(#[source] reqwest::Error),
    #[error("the model gateway did not send data in time")]
    Timeout,
    #[error("the HTTP exchange with the model gateway failed")]
    Http(#[source] reqwest::Error),
    #[error("the connection to the model gateway failed")]
    Io(#[source] std::io::Error),
}

/// Why a model call failed.
///
/// A non-2xx answer is `Status`; its `body_excerpt` is a bounded excerpt of the
/// response body and response headers are never kept.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("the model gateway answered HTTP {status}: {body_excerpt}")]
    Status {
        status: u16,
        retry_after: Option<Duration>,
        body_excerpt: String,
    },
    #[error("the conversation does not fit the model's context window: {message}")]
    ContextOverflow {
        status: Option<u16>,
        message: String,
    },
    #[error("the model stream reported an error: {message}")]
    Provider {
        kind: Option<String>,
        message: String,
    },
    #[error("the model call was aborted")]
    Aborted,
    #[error("the model transport failed")]
    Transport(#[from] TransportError),
    #[error("the model stream sent an event that is not valid JSON")]
    Decode(#[source] serde_json::Error),
    #[error("could not encode the model request")]
    Encode(#[source] serde_json::Error),
    #[error("the model endpoint URL cannot take a path")]
    InvalidEndpoint,
    #[error("the model stream ended before it finished")]
    Truncated,
}

pub(crate) fn excerpt(text: &str) -> String {
    let trimmed = text.trim();
    let mut out: String = trimmed.chars().take(MAX_EXCERPT_CHARS).collect();
    if out.len() < trimmed.len() {
        out.push('…');
    }
    out
}

pub(crate) fn stream_error(kind: Option<String>, message: Option<String>) -> LlmError {
    let message = message
        .or_else(|| kind.clone())
        .unwrap_or_else(|| "unknown stream error".to_owned());
    let probe = format!("{} {}", kind.as_deref().unwrap_or_default(), message);
    if crate::overflow::is_context_overflow(None, &probe) {
        return LlmError::ContextOverflow {
            status: None,
            message: excerpt(&message),
        };
    }
    LlmError::Provider {
        kind,
        message: excerpt(&message),
    }
}
