use super::McpToolResultMetadata;
use crate::errors::MetadataError;
use rmcp::model::CallToolResult;

/// Extension trait that extracts a strongly-typed
/// [`McpToolResultMetadata`] block from an MCP `CallToolResult`.
pub trait CallToolResultExt {
    /// Decode the `_meta` payload of this tool result.
    ///
    /// # Errors
    ///
    /// Returns [`MetadataError`] when the `_meta` block is missing or
    /// fails to deserialize.
    fn get_mcp_metadata(&self) -> Result<McpToolResultMetadata, MetadataError>;
}

impl CallToolResultExt for CallToolResult {
    fn get_mcp_metadata(&self) -> Result<McpToolResultMetadata, MetadataError> {
        McpToolResultMetadata::from_call_tool_result(self)
    }
}
