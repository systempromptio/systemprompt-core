//! Request-shape types passed into [`crate::llm::LlmProvider::chat`].

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::message::ChatMessage;
use crate::tool::ToolDefinition;

/// Sampling-parameter overrides for a single chat call.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SamplingParameters {
    /// Softmax temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Nucleus-sampling cutoff.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Top-K sampling cutoff.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
}

impl SamplingParameters {
    /// Build a [`SamplingParameters`] with no overrides applied.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            temperature: None,
            top_p: None,
            top_k: None,
        }
    }

    /// Set the [`temperature`](Self::temperature) field.
    #[must_use]
    pub const fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the [`top_p`](Self::top_p) field.
    #[must_use]
    pub const fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set the [`top_k`](Self::top_k) field.
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

/// A fully-specified chat request ready to be dispatched to an LLM provider.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// Conversation history including the new user message.
    pub messages: Vec<ChatMessage>,
    /// Provider-specific model identifier.
    pub model: String,
    /// Maximum tokens the model may emit in this turn.
    pub max_output_tokens: u32,
    /// Optional sampling-parameter overrides.
    pub sampling: Option<SamplingParameters>,
    /// Optional tool definitions made available to the model.
    pub tools: Option<Vec<ToolDefinition>>,
    /// Optional JSON schema constraining the response shape.
    pub response_schema: Option<JsonValue>,
}

impl ChatRequest {
    /// Build a minimal [`ChatRequest`] with only required fields populated.
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

    /// Attach sampling-parameter overrides.
    #[must_use]
    pub const fn with_sampling(mut self, sampling: SamplingParameters) -> Self {
        self.sampling = Some(sampling);
        self
    }

    /// Make tool definitions available to the model for this call.
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Constrain the response shape with a JSON schema.
    #[must_use]
    pub fn with_response_schema(mut self, schema: JsonValue) -> Self {
        self.response_schema = Some(schema);
        self
    }
}
