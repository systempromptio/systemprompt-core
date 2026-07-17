//! Errors raised while decoding MCP `_meta` payloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
