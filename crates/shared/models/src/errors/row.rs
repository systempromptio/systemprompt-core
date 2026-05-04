//! Errors raised while decoding `HashMap<String, serde_json::Value>`
//! row payloads produced by the runtime SQL adapter.

/// Failure to deserialize a row-shaped `HashMap<String, serde_json::Value>`
/// into a strongly-typed model (tool call, tool execution, service record).
#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum RowParseError {
    /// A required column was missing or had the wrong type.
    #[error("missing or invalid field: {0}")]
    Missing(&'static str),

    /// A numeric field was outside the representable range.
    #[error("{0}")]
    OutOfRange(&'static str),
}
