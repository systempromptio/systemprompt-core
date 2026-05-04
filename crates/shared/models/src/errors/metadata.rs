//! Errors raised while decoding MCP `_meta` payloads.

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("McpToolResultMetadata: mcp_execution_id cannot be empty")]
    MissingExecutionId,

    #[error("CallToolResult._meta is missing (required for MCP execution tracking)")]
    MetaMissing,

    #[error("Failed to serialize McpToolResultMetadata as JSON object")]
    NotJsonObject,

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}
