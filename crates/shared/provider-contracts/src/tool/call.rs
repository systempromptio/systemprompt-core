//! Tool call-request / call-result types exchanged with a
//! [`crate::tool::ToolProvider`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::content::ToolContent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub tool_call_id: String,
    pub name: String,
    // JSON: MCP tool-call arguments are the tool's own JSON object.
    pub arguments: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    pub structured_content: Option<JsonValue>,
    pub is_error: Option<bool>,
    // JSON: MCP `_meta` is an open map of vendor-prefixed keys.
    pub meta: Option<JsonValue>,
}

impl ToolCallResult {
    #[must_use]
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        }
    }

    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(message)],
            structured_content: None,
            is_error: Some(true),
            meta: None,
        }
    }

    #[must_use]
    // JSON: MCP `CallToolResult.structuredContent` is spec-defined as free-form.
    pub fn with_structured_content(mut self, content: JsonValue) -> Self {
        self.structured_content = Some(content);
        self
    }
}
