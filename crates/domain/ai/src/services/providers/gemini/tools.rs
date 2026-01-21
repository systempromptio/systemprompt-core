use anyhow::{anyhow, Result};
use std::time::Instant;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::models::ai::{AiMessage, AiResponse, SamplingParams};
use crate::models::providers::gemini::{
    GeminiContent, GeminiFunctionCall, GeminiFunctionCallingConfig, GeminiFunctionResponse,
    GeminiPart, GeminiRequest, GeminiResponse, GeminiToolConfig,
};
use crate::models::tools::{CallToolResult, ToolCall};

use super::params::ToolConfigParams;
pub use super::params::{ToolRequestParams, ToolResultParams};
use super::provider::GeminiProvider;
use super::request_builders::AiResponseParams;
use super::tool_conversion::{build_thinking_config, convert_tools, extract_tool_response};
use super::{converters, request_builders};

pub async fn generate_with_tools(
    provider: &GeminiProvider,
    params: ToolRequestParams<'_>,
) -> Result<(AiResponse, Vec<ToolCall>)> {
    let config_params = ToolConfigParams::new(&params);
    generate_with_tools_config(provider, config_params).await
}

async fn generate_with_tools_config(
    provider: &GeminiProvider,
    params: ToolConfigParams<'_>,
) -> Result<(AiResponse, Vec<ToolCall>)> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let request = build_tool_request(provider, &params)?;

    info!(
        request_id = %request_id,
        model = %params.model,
        tool_count = params.tools.len(),
        message_count = params.messages.len(),
        max_output_tokens = params.max_output_tokens,
        "Sending tool request to Gemini"
    );

    let gemini_response = send_tool_request(provider, &request, params.model, request_id).await?;

    build_tool_response(request_id, &gemini_response, provider, params.model, start).await
}

fn build_tool_request(
    provider: &GeminiProvider,
    params: &ToolConfigParams<'_>,
) -> Result<GeminiRequest> {
    let contents = converters::convert_messages(params.messages);
    let gemini_tools = convert_tools(provider, params.tools.clone())?;
    let thinking_config = build_thinking_config(params.model);
    let generation_config = request_builders::build_generation_config(
        params.sampling,
        params.max_output_tokens,
        None,
        thinking_config,
    );

    let tool_config = GeminiToolConfig {
        function_calling_config: GeminiFunctionCallingConfig {
            mode: params.function_calling_mode,
            allowed_function_names: params.allowed_function_names.clone(),
        },
    };

    Ok(GeminiRequest {
        contents,
        generation_config: Some(generation_config),
        safety_settings: None,
        tools: Some(gemini_tools),
        tool_config: Some(tool_config),
    })
}

async fn send_tool_request(
    provider: &GeminiProvider,
    request: &GeminiRequest,
    model: &str,
    request_id: Uuid,
) -> Result<GeminiResponse> {
    let response_text =
        request_builders::send_request(provider, request, model, "generateContent").await?;

    debug!(
        request_id = %request_id,
        response_length = response_text.len(),
        "Received response from Gemini"
    );

    request_builders::parse_response(&response_text).map_err(|e| {
        error!(
            request_id = %request_id,
            error = %e,
            response_preview = %response_text.chars().take(1000).collect::<String>(),
            "Failed to parse Gemini response"
        );
        e
    })
}

async fn build_tool_response(
    request_id: Uuid,
    gemini_response: &GeminiResponse,
    provider: &GeminiProvider,
    model: &str,
    start: Instant,
) -> Result<(AiResponse, Vec<ToolCall>)> {
    let (content, tool_calls) = extract_tool_response(provider, gemini_response).await?;

    info!(
        request_id = %request_id,
        has_text = !content.is_empty(),
        tool_call_count = tool_calls.len(),
        latency_ms = start.elapsed().as_millis() as u64,
        "Parsed Gemini response"
    );

    let response = request_builders::build_ai_response(
        AiResponseParams::builder(request_id, gemini_response, model, start, content).build(),
    );

    Ok((response, tool_calls))
}

pub async fn generate_with_tool_results(
    provider: &GeminiProvider,
    params: ToolResultParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let contents = build_tool_result_contents(
        params.conversation_history,
        params.tool_calls,
        params.tool_results,
    );
    let request = build_tool_result_request(contents, params.sampling, params.max_output_tokens);

    let response_text =
        request_builders::send_request(provider, &request, params.model, "generateContent").await?;

    let gemini_response: GeminiResponse = request_builders::parse_response(&response_text)?;
    let content = extract_synthesis_content(&gemini_response)?;

    debug!(
        request_id = %request_id,
        model = %params.model,
        has_content = !content.is_empty(),
        content_length = content.len(),
        tool_call_count = params.tool_calls.len(),
        tool_result_count = params.tool_results.len(),
        "Tool synthesis response details"
    );

    Ok(request_builders::build_ai_response(
        AiResponseParams::builder(request_id, &gemini_response, params.model, start, content)
            .build(),
    ))
}

fn build_tool_result_contents(
    conversation_history: &[AiMessage],
    tool_calls: &[ToolCall],
    tool_results: &[CallToolResult],
) -> Vec<GeminiContent> {
    let mut contents = converters::convert_messages(conversation_history);

    let assistant_parts: Vec<_> = tool_calls
        .iter()
        .map(|tc| GeminiPart::FunctionCall {
            function_call: GeminiFunctionCall {
                name: tc.name.clone(),
                args: tc.arguments.clone(),
                thought_signature: None,
            },
        })
        .collect();

    if !assistant_parts.is_empty() {
        contents.push(GeminiContent {
            role: "model".to_string(),
            parts: assistant_parts,
        });
    }

    let user_parts: Vec<_> = tool_calls
        .iter()
        .zip(tool_results.iter())
        .map(|(tc, tr)| GeminiPart::FunctionResponse {
            function_response: GeminiFunctionResponse {
                name: tc.name.clone(),
                response: converters::convert_tool_result_to_json(tr),
            },
        })
        .collect();

    if !user_parts.is_empty() {
        contents.push(GeminiContent {
            role: "user".to_string(),
            parts: user_parts,
        });
    }

    contents
}

fn build_tool_result_request(
    contents: Vec<GeminiContent>,
    sampling: Option<&SamplingParams>,
    max_output_tokens: u32,
) -> GeminiRequest {
    let generation_config =
        request_builders::build_generation_config(sampling, max_output_tokens, None, None);

    GeminiRequest {
        contents,
        generation_config: Some(generation_config),
        safety_settings: None,
        tools: None,
        tool_config: None,
    }
}

fn extract_synthesis_content(gemini_response: &GeminiResponse) -> Result<String> {
    let candidate = gemini_response
        .candidates
        .first()
        .ok_or_else(|| anyhow!("No response from Gemini for tool synthesis"))?;

    let finish_reason = candidate.finish_reason.as_deref().unwrap_or("UNKNOWN");

    let candidate_content = candidate.content.as_ref().ok_or_else(|| {
        anyhow!("Gemini returned no content after tool execution. Finish reason: {finish_reason}")
    })?;

    Ok(candidate_content
        .parts
        .iter()
        .filter_map(|part| match part {
            GeminiPart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect())
}
