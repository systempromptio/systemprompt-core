//! Errors raised while decoding `HashMap<String, serde_json::Value>`
//! row payloads produced by the runtime SQL adapter.

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum RowParseError {
    #[error("missing or invalid field: {0}")]
    Missing(&'static str),

    #[error("{0}")]
    OutOfRange(&'static str),
}
