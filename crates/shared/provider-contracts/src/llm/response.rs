//! Response-shape types returned by [`crate::llm::LlmProvider::chat`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::tool::ToolCallRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCallRequest>,
    pub usage: Option<TokenUsage>,
    pub model: String,
    pub latency_ms: u64,
}

impl ChatResponse {
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

    #[must_use]
    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCallRequest>) -> Self {
        self.tool_calls = tool_calls;
        self
    }

    #[must_use]
    pub const fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }

    #[must_use]
    pub const fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TokenUsage {
    #[serde(rename = "input_tokens")]
    pub input: u32,
    #[serde(rename = "output_tokens")]
    pub output: u32,
    #[serde(rename = "total_tokens")]
    pub total: u32,
    #[serde(rename = "cache_read_tokens")]
    pub cache_read: Option<u32>,
    #[serde(rename = "cache_creation_tokens")]
    pub cache_creation: Option<u32>,
}

impl TokenUsage {
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

    #[must_use]
    pub const fn with_cache_read(mut self, cache_read: u32) -> Self {
        self.cache_read = Some(cache_read);
        self
    }

    #[must_use]
    pub const fn with_cache_creation(mut self, cache_creation: u32) -> Self {
        self.cache_creation = Some(cache_creation);
        self
    }
}
