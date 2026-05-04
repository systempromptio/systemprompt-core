//! Errors raised while decoding MCP `_meta` payloads.

/// Failure to construct or validate `McpToolResultMetadata`.
#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    /// The metadata is missing the required execution id.
    #[error("McpToolResultMetadata: mcp_execution_id cannot be empty")]
    MissingExecutionId,

    /// The MCP tool result did not include a `_meta` block.
    #[error("CallToolResult._meta is missing (required for MCP execution tracking)")]
    MetaMissing,

    /// JSON serialization of metadata produced a non-object value.
    #[error("Failed to serialize McpToolResultMetadata as JSON object")]
    NotJsonObject,

    /// JSON (de)serialization failed.
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}
