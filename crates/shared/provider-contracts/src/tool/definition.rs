//! [`ToolDefinition`] — one tool exposed by a [`crate::tool::ToolProvider`].

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// A single tool exposed by a [`crate::tool::ToolProvider`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ToolDefinition {
    /// Tool name as exposed to the model.
    pub name: String,
    /// Human-readable description shown to the model.
    pub description: Option<String>,
    /// JSON schema describing the tool's expected input arguments.
    pub input_schema: Option<JsonValue>,
    /// JSON schema describing the tool's structured output.
    pub output_schema: Option<JsonValue>,
    /// Identifier of the backing service (typically an MCP server id).
    pub service_id: String,
    /// When `true`, a successful call ends the agent turn.
    #[serde(default)]
    pub terminal_on_success: bool,
    /// Optional model-specific overrides keyed by provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_config: Option<JsonValue>,
}

impl ToolDefinition {
    /// Build a [`ToolDefinition`] with only the required fields populated.
    #[must_use]
    pub fn new(name: impl Into<String>, service_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: None,
            output_schema: None,
            service_id: service_id.into(),
            terminal_on_success: false,
            model_config: None,
        }
    }

    /// Set the [`description`](Self::description) field.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the [`input_schema`](Self::input_schema) field.
    #[must_use]
    pub fn with_input_schema(mut self, schema: JsonValue) -> Self {
        self.input_schema = Some(schema);
        self
    }

    /// Set the [`output_schema`](Self::output_schema) field.
    #[must_use]
    pub fn with_output_schema(mut self, schema: JsonValue) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// Set the [`terminal_on_success`](Self::terminal_on_success) flag.
    #[must_use]
    pub const fn with_terminal_on_success(mut self, terminal: bool) -> Self {
        self.terminal_on_success = terminal;
        self
    }

    /// Set the [`model_config`](Self::model_config) field.
    #[must_use]
    pub fn with_model_config(mut self, config: JsonValue) -> Self {
        self.model_config = Some(config);
        self
    }
}
