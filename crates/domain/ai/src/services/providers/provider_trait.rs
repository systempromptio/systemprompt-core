use crate::models::ai::{
    AiMessage, AiResponse, ResponseFormat, SamplingParams, SearchGroundedResponse,
};
use crate::models::tools::{CallToolResult, McpTool, ToolCall};
use crate::services::schema::ProviderCapabilities;
use anyhow::Result;
use async_trait::async_trait;
use futures::stream::Stream;
use rmcp::model::RawContent;
use std::pin::Pin;

#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    pub input_cost_per_1k: f32,
    pub output_cost_per_1k: f32,
}

impl ModelPricing {
    pub const fn new(input_cost_per_1k: f32, output_cost_per_1k: f32) -> Self {
        Self {
            input_cost_per_1k,
            output_cost_per_1k,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GenerationParams<'a> {
    pub messages: &'a [AiMessage],
    pub model: &'a str,
    pub max_output_tokens: u32,
    pub sampling: Option<&'a SamplingParams>,
}

impl<'a> GenerationParams<'a> {
    pub const fn new(messages: &'a [AiMessage], model: &'a str, max_output_tokens: u32) -> Self {
        Self {
            messages,
            model,
            max_output_tokens,
            sampling: None,
        }
    }

    pub const fn with_sampling(mut self, sampling: &'a SamplingParams) -> Self {
        self.sampling = Some(sampling);
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
    pub response_schema: serde_json::Value,
}

impl<'a> SchemaGenerationParams<'a> {
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

    pub fn with_response_schema(mut self, schema: serde_json::Value) -> Self {
        self.response_schema = Some(schema);
        self
    }
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &str;

    fn as_any(&self) -> &dyn std::any::Any;

    fn capabilities(&self) -> ProviderCapabilities;

    fn supports_model(&self, model: &str) -> bool;

    fn supports_sampling(&self, sampling: Option<&SamplingParams>) -> bool;

    fn default_model(&self) -> &str;

    fn get_pricing(&self, model: &str) -> ModelPricing;

    async fn generate(&self, params: GenerationParams<'_>) -> Result<AiResponse>;

    async fn generate_with_tools(
        &self,
        params: ToolGenerationParams<'_>,
    ) -> Result<(AiResponse, Vec<ToolCall>)>;

    async fn generate_with_tool_results(
        &self,
        params: ToolResultsParams<'_>,
    ) -> Result<AiResponse> {
        let mut messages = params.base.messages.to_vec();

        let mut tool_summary = String::new();
        for (call, result) in params.tool_calls.iter().zip(params.tool_results.iter()) {
            let content_text: String = result
                .content
                .iter()
                .filter_map(|c| match &c.raw {
                    RawContent::Text(text_content) => Some(text_content.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            if result.is_error.unwrap_or(false) {
                tool_summary.push_str(&format!("Tool {} failed: {}\n", call.name, content_text));
            } else {
                tool_summary.push_str(&format!("Tool {} result: {}\n", call.name, content_text));
            }
        }

        messages.push(AiMessage {
            role: crate::models::ai::MessageRole::User,
            content: format!(
                "Based on the tool results above, please provide a helpful response to the \
                 original question:\n\n{tool_summary}"
            ),
            parts: Vec::new(),
        });

        let gen_params = GenerationParams {
            messages: &messages,
            model: params.base.model,
            max_output_tokens: params.base.max_output_tokens,
            sampling: params.base.sampling,
        };
        self.generate(gen_params).await
    }

    async fn generate_structured(
        &self,
        params: StructuredGenerationParams<'_>,
    ) -> Result<AiResponse> {
        self.generate(params.base).await
    }

    async fn generate_with_schema(&self, params: SchemaGenerationParams<'_>) -> Result<AiResponse>;

    fn supports_json_mode(&self) -> bool {
        false
    }

    fn supports_structured_output(&self) -> bool {
        true
    }

    async fn generate_stream(
        &self,
        _params: GenerationParams<'_>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        Err(anyhow::anyhow!(
            "Streaming not supported by provider {}",
            self.name()
        ))
    }

    async fn generate_with_tools_stream(
        &self,
        _params: ToolGenerationParams<'_>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        Err(anyhow::anyhow!(
            "Tool streaming not supported by provider {}",
            self.name()
        ))
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn supports_google_search(&self) -> bool {
        false
    }

    async fn generate_with_google_search(
        &self,
        _params: SearchGenerationParams<'_>,
    ) -> Result<SearchGroundedResponse> {
        Err(anyhow::anyhow!(
            "Google Search not supported by provider {}",
            self.name()
        ))
    }
}
