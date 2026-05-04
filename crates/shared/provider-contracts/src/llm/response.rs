//! Response-shape types returned by [`crate::llm::LlmProvider::chat`].

use serde::{Deserialize, Serialize};

use crate::tool::ToolCallRequest;

/// A single chat-completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Assistant-generated text.
    pub content: String,
    /// Tool calls the assistant requested be executed.
    pub tool_calls: Vec<ToolCallRequest>,
    /// Token-usage accounting, when reported by the provider.
    pub usage: Option<TokenUsage>,
    /// Resolved provider-specific model identifier.
    pub model: String,
    /// End-to-end provider latency in milliseconds.
    pub latency_ms: u64,
}

impl ChatResponse {
    /// Build a [`ChatResponse`] with content + model only.
    #[must_use]
    pub fn new(content: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_calls: vec![],
            usage: None,
            model: model.into(),
            latency_ms: 0,
        }
    }

    /// Attach tool-call requests to this response.
    #[must_use]
    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCallRequest>) -> Self {
        self.tool_calls = tool_calls;
        self
    }

    /// Attach token-usage accounting.
    #[must_use]
    pub const fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }

    /// Record observed end-to-end latency.
    #[must_use]
    pub const fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }
}

/// Token-usage accounting for a single chat call.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Tokens consumed by the prompt.
    #[serde(rename = "input_tokens")]
    pub input: u32,
    /// Tokens emitted by the model.
    #[serde(rename = "output_tokens")]
    pub output: u32,
    /// Sum of input and output tokens.
    #[serde(rename = "total_tokens")]
    pub total: u32,
    /// Tokens served from the prompt cache, when reported.
    #[serde(rename = "cache_read_tokens")]
    pub cache_read: Option<u32>,
    /// Tokens written to the prompt cache, when reported.
    #[serde(rename = "cache_creation_tokens")]
    pub cache_creation: Option<u32>,
}

impl TokenUsage {
    /// Build a [`TokenUsage`] with the totals derived from `input + output`.
    #[must_use]
    pub const fn new(input: u32, output: u32) -> Self {
        Self {
            input,
            output,
            total: input + output,
            cache_read: None,
            cache_creation: None,
        }
    }

    /// Set the [`cache_read`](Self::cache_read) field.
    #[must_use]
    pub const fn with_cache_read(mut self, cache_read: u32) -> Self {
        self.cache_read = Some(cache_read);
        self
    }

    /// Set the [`cache_creation`](Self::cache_creation) field.
    #[must_use]
    pub const fn with_cache_creation(mut self, cache_creation: u32) -> Self {
        self.cache_creation = Some(cache_creation);
        self
    }
}
