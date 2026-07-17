//! Typed `_meta` payload carried on MCP tool results.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use systemprompt_identifiers::McpExecutionId;

use crate::errors::MetadataError;

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

    pub fn validate(&self) -> Result<(), MetadataError> {
        if self.mcp_execution_id.as_str().is_empty() {
            return Err(MetadataError::MissingExecutionId);
        }
        Ok(())
    }

    pub fn to_meta(&self) -> Result<rmcp::model::Meta, MetadataError> {
        self.validate()?;

        let json_value = serde_json::to_value(self)?;
        let json_object = json_value
            .as_object()
            .ok_or(MetadataError::NotJsonObject)?
            .clone();

        Ok(rmcp::model::Meta(json_object))
    }

    pub fn from_meta(meta: &rmcp::model::Meta) -> Result<Self, MetadataError> {
        let json_value = Value::Object(meta.0.clone());
        let metadata: Self = serde_json::from_value(json_value)?;

        metadata.validate()?;
        Ok(metadata)
    }

    pub fn from_call_tool_result(
        result: &rmcp::model::CallToolResult,
    ) -> Result<Self, MetadataError> {
        let meta = result.meta.as_ref().ok_or(MetadataError::MetaMissing)?;

        Self::from_meta(meta)
    }
}
