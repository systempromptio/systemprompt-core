//! [`ToolDefinition`] — one tool exposed by a [`crate::tool::ToolProvider`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use systemprompt_identifiers::McpServerId;

use super::model_config::ToolModelConfig;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    // JSON: MCP `Tool.inputSchema` / `outputSchema` are JSON Schema documents
    // owned by the server; the protocol defines them as free-form objects.
    pub input_schema: Option<JsonValue>,
    pub output_schema: Option<JsonValue>,
    pub service_id: McpServerId,
    #[serde(default)]
    pub terminal_on_success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_config: Option<ToolModelConfig>,
}

impl ToolDefinition {
    #[must_use]
    pub fn new(name: impl Into<String>, service_id: McpServerId) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: None,
            output_schema: None,
            service_id,
            terminal_on_success: false,
            model_config: None,
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    // JSON: MCP `inputSchema`/`outputSchema` JSON Schema owned by the server.
    pub fn with_input_schema(mut self, schema: JsonValue) -> Self {
        self.input_schema = Some(schema);
        self
    }

    #[must_use]
    // JSON: MCP `inputSchema`/`outputSchema` JSON Schema owned by the server.
    pub fn with_output_schema(mut self, schema: JsonValue) -> Self {
        self.output_schema = Some(schema);
        self
    }

    #[must_use]
    pub const fn with_terminal_on_success(mut self, terminal: bool) -> Self {
        self.terminal_on_success = terminal;
        self
    }

    #[must_use]
    pub fn with_model_config(mut self, config: ToolModelConfig) -> Self {
        self.model_config = Some(config);
        self
    }
}
