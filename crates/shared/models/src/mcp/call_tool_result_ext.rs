//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::McpToolResultMetadata;
use crate::errors::MetadataError;
use rmcp::model::CallToolResult;

pub trait CallToolResultExt {
    fn get_mcp_metadata(&self) -> Result<McpToolResultMetadata, MetadataError>;
}

impl CallToolResultExt for CallToolResult {
    fn get_mcp_metadata(&self) -> Result<McpToolResultMetadata, MetadataError> {
        McpToolResultMetadata::from_call_tool_result(self)
    }
}
