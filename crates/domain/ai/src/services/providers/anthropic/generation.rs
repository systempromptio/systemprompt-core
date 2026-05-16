use crate::error::Result;
use std::time::Instant;
use uuid::Uuid;

use crate::models::ai::AiResponse;
use crate::models::providers::anthropic::{
    AnthropicContentBlock, AnthropicRequest, AnthropicResponse, AnthropicTool, AnthropicToolChoice,
};
use crate::models::tools::ToolCall;
use crate::services::providers::{GenerationParams, SchemaGenerationParams, ToolGenerationParams};
use systemprompt_identifiers::AiToolCallId;

use super::provider::AnthropicProvider;
use super::request::{post_messages, sampling_tuple};
use super::response::{ResponseContext, build_response};
use super::{converters, thinking};

pub async fn generate(
    provider: &AnthropicProvider,
    params: GenerationParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let (system_prompt, anthropic_messages) = converters::convert_messages(params.messages);
    let (temperature, top_p, top_k, stop_sequences) = sampling_tuple(params.sampling);

    let request = AnthropicRequest {
        model: params.model.to_string(),
        messages: anthropic_messages,
        max_tokens: params.max_output_tokens,
        temperature,
        top_p,
        top_k,
        stop_sequences,
        system: system_prompt,
        tools: None,
        tool_choice: None,
        stream: None,
        thinking: thinking::build_thinking_config(params.model),
    };

    let anthropic_response: AnthropicResponse =
        post_messages(provider, &request).await?.json().await?;

    let content = anthropic_response
        .content
        .iter()
        .filter_map(|block| match block {
            AnthropicContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<String>();

    Ok(build_response(
        ResponseContext {
            request_id,
            model: params.model,
            start,
        },
        &anthropic_response,
        content,
    ))
}

pub async fn generate_with_tools(
    provider: &AnthropicProvider,
    params: ToolGenerationParams<'_>,
) -> Result<(AiResponse, Vec<ToolCall>)> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let (system_prompt, anthropic_messages) = converters::convert_messages(params.base.messages);
    let anthropic_tools = converters::convert_tools(params.tools);
    let (temperature, top_p, top_k, stop_sequences) = sampling_tuple(params.base.sampling);

    let request = AnthropicRequest {
        model: params.base.model.to_string(),
        messages: anthropic_messages,
        max_tokens: params.base.max_output_tokens,
        temperature,
        top_p,
        top_k,
        stop_sequences,
        system: system_prompt,
        tools: Some(anthropic_tools),
        tool_choice: None,
        stream: None,
        thinking: thinking::build_thinking_config(params.base.model),
    };

    let anthropic_response: AnthropicResponse =
        post_messages(provider, &request).await?.json().await?;

    let (content, tool_calls) = split_content_and_tools(&anthropic_response);

    let ai_response = build_response(
        ResponseContext {
            request_id,
            model: params.base.model,
            start,
        },
        &anthropic_response,
        content,
    );

    Ok((ai_response, tool_calls))
}

pub async fn generate_with_schema(
    provider: &AnthropicProvider,
    params: SchemaGenerationParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let (system_prompt, anthropic_messages) = converters::convert_messages(params.base.messages);
    let (temperature, top_p, top_k, stop_sequences) = sampling_tuple(params.base.sampling);

    let structured_tool = AnthropicTool {
        name: "structured_output".to_string(),
        description: Some("Return structured JSON output matching the schema".to_string()),
        input_schema: params.response_schema,
    };

    let request = AnthropicRequest {
        model: params.base.model.to_string(),
        messages: anthropic_messages,
        max_tokens: params.base.max_output_tokens,
        temperature,
        top_p,
        top_k,
        stop_sequences,
        system: system_prompt,
        tools: Some(vec![structured_tool]),
        tool_choice: Some(AnthropicToolChoice::Tool {
            name: "structured_output".to_string(),
        }),
        stream: None,
        thinking: thinking::build_thinking_config(params.base.model),
    };

    let anthropic_response: AnthropicResponse =
        post_messages(provider, &request).await?.json().await?;

    let content = anthropic_response
        .content
        .iter()
        .find_map(|block| match block {
            AnthropicContentBlock::ToolUse { input, .. } => match serde_json::to_string(input) {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to serialize Anthropic tool input");
                    Some(String::new())
                },
            },
            _ => None,
        })
        .unwrap_or_default();

    Ok(build_response(
        ResponseContext {
            request_id,
            model: params.base.model,
            start,
        },
        &anthropic_response,
        content,
    ))
}

fn split_content_and_tools(response: &AnthropicResponse) -> (String, Vec<ToolCall>) {
    let mut content = String::new();
    let mut tool_calls = Vec::new();

    for block in &response.content {
        match block {
            AnthropicContentBlock::Text { text } => content.push_str(text),
            AnthropicContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall {
                    ai_tool_call_id: AiToolCallId::new(id.clone()),
                    name: name.clone(),
                    arguments: input.clone(),
                });
            },
            AnthropicContentBlock::Image { .. } | AnthropicContentBlock::ToolResult { .. } => {},
        }
    }

    (content, tool_calls)
}
