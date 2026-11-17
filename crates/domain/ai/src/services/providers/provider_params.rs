//! Borrowed request parameter structs handed to a
//! [`ProviderClient`](super::provider_trait::ProviderClient) call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{AiError, RequestError, Result};
use crate::models::ai::{AiMessage, AiRequest, ReasoningParams, ResponseFormat, SamplingParams};
use crate::models::tools::{CallToolResult, McpTool, ToolCall};

#[derive(Debug, Clone)]
pub struct GenerationParams<'a> {
    pub messages: &'a [AiMessage],
    pub model: &'a str,
    pub max_output_tokens: u32,
    pub sampling: Option<&'a SamplingParams>,
    pub reasoning: Option<ReasoningParams>,
}

impl<'a> GenerationParams<'a> {
    pub const fn new(messages: &'a [AiMessage], model: &'a str, max_output_tokens: u32) -> Self {
        Self {
            messages,
            model,
            max_output_tokens,
            sampling: None,
            reasoning: None,
        }
    }

    pub fn from_request(request: &'a AiRequest, model: &'a str) -> Result<Self> {
        let max_output_tokens = request.max_output_tokens();
        if let Some(budget) = request.reasoning.and_then(|r| r.thinking_budget_tokens)
            && budget >= max_output_tokens
        {
            return Err(AiError::Request(RequestError::ThinkingBudgetTooLarge {
                budget,
                max_output_tokens,
            }));
        }
        Ok(Self {
            messages: &request.messages,
            model,
            max_output_tokens,
            sampling: request.sampling.as_ref(),
            reasoning: request.reasoning,
        })
    }

    pub const fn with_sampling(mut self, sampling: &'a SamplingParams) -> Self {
        self.sampling = Some(sampling);
        self
    }

    pub const fn with_reasoning(mut self, reasoning: ReasoningParams) -> Self {
        self.reasoning = Some(reasoning);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ToolGenerationParams<'a> {
    pub base: GenerationParams<'a>,
    pub tools: Vec<McpTool>,
}

impl<'a> ToolGenerationParams<'a> {
    pub const fn new(base: GenerationParams<'a>, tools: Vec<McpTool>) -> Self {
        Self { base, tools }
    }
}

#[derive(Debug, Clone)]
pub struct ToolResultsParams<'a> {
    pub base: GenerationParams<'a>,
    pub tool_calls: &'a [ToolCall],
    pub tool_results: &'a [CallToolResult],
}

impl<'a> ToolResultsParams<'a> {
    pub const fn new(
        base: GenerationParams<'a>,
        tool_calls: &'a [ToolCall],
        tool_results: &'a [CallToolResult],
    ) -> Self {
        Self {
            base,
            tool_calls,
            tool_results,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SchemaGenerationParams<'a> {
    pub base: GenerationParams<'a>,
    // JSON: arbitrary caller-supplied JSON Schema
    pub response_schema: serde_json::Value,
}

impl<'a> SchemaGenerationParams<'a> {
    // JSON: JSON Schema for structured output — caller-supplied, arbitrary.
    pub const fn new(base: GenerationParams<'a>, response_schema: serde_json::Value) -> Self {
        Self {
            base,
            response_schema,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructuredGenerationParams<'a> {
    pub base: GenerationParams<'a>,
    pub response_format: &'a ResponseFormat,
}

impl<'a> StructuredGenerationParams<'a> {
    pub const fn new(base: GenerationParams<'a>, response_format: &'a ResponseFormat) -> Self {
        Self {
            base,
            response_format,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchGenerationParams<'a> {
    pub base: GenerationParams<'a>,
    pub urls: Option<Vec<String>>,
    // JSON: arbitrary caller-supplied JSON Schema
    pub response_schema: Option<serde_json::Value>,
}

impl<'a> SearchGenerationParams<'a> {
    pub const fn new(base: GenerationParams<'a>) -> Self {
        Self {
            base,
            urls: None,
            response_schema: None,
        }
    }

    pub fn with_urls(mut self, urls: Vec<String>) -> Self {
        self.urls = Some(urls);
        self
    }

    // JSON: JSON Schema for structured output — caller-supplied, arbitrary.
    pub fn with_response_schema(mut self, schema: serde_json::Value) -> Self {
        self.response_schema = Some(schema);
        self
    }
}
