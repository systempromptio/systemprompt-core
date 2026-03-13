use super::McpToolResultMetadata;
use anyhow::Result;
use rmcp::model::CallToolResult;

pub trait CallToolResultExt {
    fn get_mcp_metadata(&self) -> Result<McpToolResultMetadata>;
}

impl CallToolResultExt for CallToolResult {
    fn get_mcp_metadata(&self) -> Result<McpToolResultMetadata> {
        McpToolResultMetadata::from_call_tool_result(self)
    }
}
