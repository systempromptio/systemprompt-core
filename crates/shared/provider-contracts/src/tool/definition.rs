//! [`ToolDefinition`] — one tool exposed by a [`crate::tool::ToolProvider`].

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<JsonValue>,
    pub output_schema: Option<JsonValue>,
    pub service_id: String,
    #[serde(default)]
    pub terminal_on_success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_config: Option<JsonValue>,
}

impl ToolDefinition {
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

    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn with_input_schema(mut self, schema: JsonValue) -> Self {
        self.input_schema = Some(schema);
        self
    }

    #[must_use]
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
    pub fn with_model_config(mut self, config: JsonValue) -> Self {
        self.model_config = Some(config);
        self
    }
}
