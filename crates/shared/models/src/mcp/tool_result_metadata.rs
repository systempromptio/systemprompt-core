use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use systemprompt_identifiers::McpExecutionId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpToolResultMetadata {
    pub mcp_execution_id: McpExecutionId,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time_ms: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
}

impl McpToolResultMetadata {
    pub const fn new(mcp_execution_id: McpExecutionId) -> Self {
        Self {
            mcp_execution_id,
            execution_time_ms: None,
            server_version: None,
        }
    }

    pub const fn with_execution_time(mut self, time_ms: i64) -> Self {
        self.execution_time_ms = Some(time_ms);
        self
    }

    pub fn with_server_version(mut self, version: impl Into<String>) -> Self {
        self.server_version = Some(version.into());
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.mcp_execution_id.as_str().is_empty() {
            return Err(anyhow!(
                "McpToolResultMetadata: mcp_execution_id cannot be empty"
            ));
        }
        Ok(())
    }

    pub fn to_meta(&self) -> Result<rmcp::model::Meta> {
        self.validate()?;

        let json_value = serde_json::to_value(self)?;
        let json_object = json_value
            .as_object()
            .ok_or_else(|| anyhow!("Failed to serialize McpToolResultMetadata as JSON object"))?
            .clone();

        Ok(rmcp::model::Meta(json_object))
    }

    pub fn from_meta(meta: &rmcp::model::Meta) -> Result<Self> {
        let json_value = Value::Object(meta.0.clone());
        let metadata: Self = serde_json::from_value(json_value).map_err(|e| {
            anyhow!(
                "Failed to parse McpToolResultMetadata from _meta: {e}. Expected fields: \
                 mcp_execution_id (required), execution_time_ms (optional), server_version \
                 (optional)"
            )
        })?;

        metadata.validate()?;
        Ok(metadata)
    }

    pub fn from_call_tool_result(result: &rmcp::model::CallToolResult) -> Result<Self> {
        let meta = result.meta.as_ref().ok_or_else(|| {
            anyhow!("CallToolResult._meta is missing (required for MCP execution tracking)")
        })?;

        Self::from_meta(meta)
    }
}
