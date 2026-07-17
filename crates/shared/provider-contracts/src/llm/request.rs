//! Request-shape types passed into [`crate::llm::LlmProvider::chat`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::message::ChatMessage;
use crate::tool::ToolDefinition;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SamplingParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
}

impl SamplingParameters {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            temperature: None,
            top_p: None,
            top_k: None,
        }
    }

    #[must_use]
    pub const fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    #[must_use]
    pub const fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    #[must_use]
    pub const fn with_top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }
}

impl Default for SamplingParameters {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub model: String,
    pub max_output_tokens: u32,
    pub sampling: Option<SamplingParameters>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub response_schema: Option<JsonValue>,
}

impl ChatRequest {
    #[must_use]
    pub fn new(
        messages: Vec<ChatMessage>,
        model: impl Into<String>,
        max_output_tokens: u32,
    ) -> Self {
        Self {
            messages,
            model: model.into(),
            max_output_tokens,
            sampling: None,
            tools: None,
            response_schema: None,
        }
    }

    #[must_use]
    pub const fn with_sampling(mut self, sampling: SamplingParameters) -> Self {
        self.sampling = Some(sampling);
        self
    }

    #[must_use]
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    #[must_use]
    pub fn with_response_schema(mut self, schema: JsonValue) -> Self {
        self.response_schema = Some(schema);
        self
    }
}
