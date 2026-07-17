//! Errors raised while decoding `HashMap<String, serde_json::Value>`
//! row payloads produced by the runtime SQL adapter.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum RowParseError {
    #[error("missing or invalid field: {0}")]
    Missing(&'static str),

    #[error("{0}")]
    OutOfRange(&'static str),
}
