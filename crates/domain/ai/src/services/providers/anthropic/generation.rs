use anyhow::{anyhow, Result};
use std::time::Instant;
use uuid::Uuid;

use crate::models::ai::{AiMessage, AiResponse, SamplingParams};
use crate::models::providers::anthropic::{
    AnthropicContentBlock, AnthropicRequest, AnthropicResponse, AnthropicTool, AnthropicToolChoice,
};
use crate::models::tools::{McpTool, ToolCall};
use systemprompt_identifiers::AiToolCallId;

use super::converters;
use super::provider::AnthropicProvider;

pub struct ToolGenerationParams<'a> {
    pub messages: &'a [AiMessage],
    pub tools: Vec<McpTool>,
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
}

pub struct SchemaGenerationParams<'a> {
    pub messages: &'a [AiMessage],
    pub response_schema: serde_json::Value,
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
}

pub async fn generate(
    provider: &AnthropicProvider,
    messages: &[AiMessage],
    sampling: Option<&SamplingParams>,
    max_output_tokens: u32,
    model: &str,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let (system_prompt, anthropic_messages) = converters::convert_messages(messages);

    let (temperature, top_p, top_k, stop_sequences) = sampling
        .map_or((None, None, None, None), |s| {
            (s.temperature, s.top_p, s.top_k, s.stop_sequences.clone())
        });

    let request = AnthropicRequest {
        model: model.to_string(),
        messages: anthropic_messages,
        max_tokens: max_output_tokens,
        temperature,
        top_p,
        top_k,
        stop_sequences,
        system: system_prompt,
        tools: None,
        tool_choice: None,
    };

    let response = provider
        .client
        .post(format!("{}/messages", provider.endpoint))
        .header("x-api-key", &provider.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("Anthropic API error: {error_text}"));
    }

    let anthropic_response: AnthropicResponse = response.json().await?;
    Ok(build_response(
        request_id,
        &anthropic_response,
        "anthropic",
        model,
        start,
    ))
}

pub async fn generate_with_tools(
    provider: &AnthropicProvider,
    params: ToolGenerationParams<'_>,
) -> Result<(AiResponse, Vec<ToolCall>)> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let (system_prompt, anthropic_messages) = converters::convert_messages(params.messages);
    let anthropic_tools = converters::convert_tools(params.tools);

    let (temperature, top_p, top_k, stop_sequences) =
        params.sampling.map_or((None, None, None, None), |s| {
            (s.temperature, s.top_p, s.top_k, s.stop_sequences.clone())
        });

    let request = AnthropicRequest {
        model: params.model.to_string(),
        messages: anthropic_messages,
        max_tokens: params.max_output_tokens,
        temperature,
        top_p,
        top_k,
        stop_sequences,
        system: system_prompt,
        tools: Some(anthropic_tools),
        tool_choice: None,
    };

    let response = provider
        .client
        .post(format!("{}/messages", provider.endpoint))
        .header("x-api-key", &provider.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("Anthropic API error: {error_text}"));
    }

    let anthropic_response: AnthropicResponse = response.json().await?;

    let mut content = String::new();
    let mut tool_calls = Vec::new();

    for block in &anthropic_response.content {
        match block {
            AnthropicContentBlock::Text { text } => {
                content.push_str(text);
            },
            AnthropicContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall {
                    ai_tool_call_id: AiToolCallId::from(id.clone()),
                    name: name.clone(),
                    arguments: input.clone(),
                });
            },
            AnthropicContentBlock::ToolResult { .. } => {},
        }
    }

    let usage = &anthropic_response.usage;
    let tokens_used = Some(usage.input + usage.output);
    let cache_hit = usage.cache_read.is_some_and(|t| t > 0);

    let ai_response = AiResponse {
        request_id,
        content,
        provider: "anthropic".to_string(),
        model: params.model.to_string(),
        finish_reason: anthropic_response.stop_reason.clone(),
        tokens_used,
        input_tokens: Some(usage.input),
        output_tokens: Some(usage.output),
        cache_hit,
        cache_read_tokens: usage.cache_read,
        cache_creation_tokens: usage.cache_creation,
        is_streaming: false,
        latency_ms: start.elapsed().as_millis() as u64,
        tool_calls: Vec::new(),
        tool_results: Vec::new(),
    };

    Ok((ai_response, tool_calls))
}

pub async fn generate_with_schema(
    provider: &AnthropicProvider,
    params: SchemaGenerationParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let (system_prompt, anthropic_messages) = converters::convert_messages(params.messages);

    let structured_tool = AnthropicTool {
        name: "structured_output".to_string(),
        description: Some("Return structured JSON output matching the schema".to_string()),
        input_schema: params.response_schema,
    };

    let (temperature, top_p, top_k, stop_sequences) =
        params.sampling.map_or((None, None, None, None), |s| {
            (s.temperature, s.top_p, s.top_k, s.stop_sequences.clone())
        });

    let request = AnthropicRequest {
        model: params.model.to_string(),
        messages: anthropic_messages,
        max_tokens: params.max_output_tokens,
        temperature,
        top_p,
        top_k,
        stop_sequences,
        system: system_prompt,
        tools: Some(vec![structured_tool]),
        tool_choice: Some(AnthropicToolChoice::Tool {
            name: "structured_output".to_string(),
        }),
    };

    let response = provider
        .client
        .post(format!("{}/messages", provider.endpoint))
        .header("x-api-key", &provider.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("Anthropic API error: {error_text}"));
    }

    let anthropic_response: AnthropicResponse = response.json().await?;

    let content = anthropic_response
        .content
        .iter()
        .find_map(|block| match block {
            AnthropicContentBlock::ToolUse { input, .. } => {
                Some(serde_json::to_string(input).unwrap_or_default())
            },
            _ => None,
        })
        .unwrap_or_default();

    let usage = &anthropic_response.usage;
    let tokens_used = Some(usage.input + usage.output);
    let cache_hit = usage.cache_read.is_some_and(|t| t > 0);

    Ok(AiResponse {
        request_id,
        content,
        provider: "anthropic".to_string(),
        model: params.model.to_string(),
        finish_reason: anthropic_response.stop_reason,
        tokens_used,
        input_tokens: Some(usage.input),
        output_tokens: Some(usage.output),
        cache_hit,
        cache_read_tokens: usage.cache_read,
        cache_creation_tokens: usage.cache_creation,
        is_streaming: false,
        latency_ms: start.elapsed().as_millis() as u64,
        tool_calls: Vec::new(),
        tool_results: Vec::new(),
    })
}

fn build_response(
    request_id: Uuid,
    anthropic_response: &AnthropicResponse,
    provider_name: &str,
    model: &str,
    start: Instant,
) -> AiResponse {
    let content = anthropic_response
        .content
        .iter()
        .filter_map(|block| match block {
            AnthropicContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<String>();

    let usage = &anthropic_response.usage;
    let tokens_used = Some(usage.input + usage.output);
    let cache_hit = usage.cache_read.is_some_and(|t| t > 0);

    AiResponse {
        request_id,
        content,
        provider: provider_name.to_string(),
        model: model.to_string(),
        finish_reason: anthropic_response.stop_reason.clone(),
        tokens_used,
        input_tokens: Some(usage.input),
        output_tokens: Some(usage.output),
        cache_hit,
        cache_read_tokens: usage.cache_read,
        cache_creation_tokens: usage.cache_creation,
        is_streaming: false,
        latency_ms: start.elapsed().as_millis() as u64,
        tool_calls: Vec::new(),
        tool_results: Vec::new(),
    }
}
