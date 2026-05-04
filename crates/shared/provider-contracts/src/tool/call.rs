//! Tool call-request / call-result types exchanged with a
//! [`crate::tool::ToolProvider`].

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::content::ToolContent;

/// A tool call requested by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    /// Provider-issued unique id used to correlate request and result.
    pub tool_call_id: String,
    /// Name of the tool the model wants to invoke.
    pub name: String,
    /// JSON arguments per the tool's input schema.
    pub arguments: JsonValue,
}

/// Result of executing a [`ToolCallRequest`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// Mixed-content tool output to feed back to the model.
    pub content: Vec<ToolContent>,
    /// Optional structured payload mirroring the unstructured content.
    pub structured_content: Option<JsonValue>,
    /// `true` when the tool call failed; preserves error context.
    pub is_error: Option<bool>,
    /// Free-form metadata attached by the tool service.
    pub meta: Option<JsonValue>,
}

impl ToolCallResult {
    /// Build a successful single-text-fragment result.
    #[must_use]
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        }
    }

    /// Build a failure result whose body is a single error message.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(message)],
            structured_content: None,
            is_error: Some(true),
            meta: None,
        }
    }

    /// Attach a structured-content mirror to this result.
    #[must_use]
    pub fn with_structured_content(mut self, content: JsonValue) -> Self {
        self.structured_content = Some(content);
        self
    }
}
