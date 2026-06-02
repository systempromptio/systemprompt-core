//! Gemini tool use and tool-result synthesis.
//!
//! Tool requests run schemas through the transformer/name-mapper, render via
//! the shared codec, and resolve model-emitted function names on the way back.
//! Tool-result synthesis appends the assistant `functionCall` turn and the
//! `functionResponse` turn to the canonical request the codec renders.

use std::time::Instant;

use rmcp::model::RawContent;
use serde_json::Value;
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalToolChoice, Role, SearchConfig,
};
use systemprompt_models::wire::gemini;
use uuid::Uuid;

use crate::error::Result;
use crate::models::ai::AiResponse;
use crate::models::tools::{CallToolResult, ToolCall};
use crate::services::providers::canonical_bridge::{self, BridgeProvider, CanonicalBuild};

pub use super::params::{ToolRequestParams, ToolResultParams};
use super::provider::GeminiProvider;
use super::tool_conversion::{convert_tools, resolve_response, thinking_for};
use super::transport;

pub(super) async fn generate_with_tools(
    provider: &GeminiProvider,
    params: ToolRequestParams<'_>,
) -> Result<(AiResponse, Vec<ToolCall>)> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let canonical_tools = convert_tools(provider, params.tools.to_vec()).await?;
    let has_tools = !canonical_tools.is_empty();

    let mut build = CanonicalBuild::new(
        BridgeProvider::Gemini,
        params.messages,
        params.model,
        params.max_output_tokens,
    )
    .with_sampling(params.sampling)
    .with_tools(canonical_tools);
    if has_tools {
        build = build.with_tool_choice(Some(CanonicalToolChoice::Auto));
    } else if provider.has_google_search() {
        build = build.with_search(Some(SearchConfig::default()));
    }
    let mut canonical = build.into_request();
    canonical.thinking = thinking_for(params.model);

    let body = gemini::build_request_body(&canonical, None);
    let value = transport::post(provider, &body, params.model, false)
        .await?
        .json()
        .await?;
    let parsed = gemini::parse_response(&value, params.model);
    let (content, tool_calls) = resolve_response(provider, &parsed).await;

    let mut response =
        canonical_bridge::to_ai_response("gemini", params.model, request_id, start, &parsed);
    response.content = content;
    response.tool_calls.clone_from(&tool_calls);
    Ok((response, tool_calls))
}

pub(super) async fn generate_with_tool_results(
    provider: &GeminiProvider,
    params: ToolResultParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let mut canonical = CanonicalBuild::new(
        BridgeProvider::Gemini,
        params.conversation_history,
        params.model,
        params.max_output_tokens,
    )
    .with_sampling(params.sampling)
    .into_request();

    let assistant: Vec<CanonicalContent> = params
        .tool_calls
        .iter()
        .map(|tc| CanonicalContent::ToolUse {
            id: tc.ai_tool_call_id.as_str().to_owned(),
            name: tc.name.clone(),
            input: tc.arguments.clone(),
        })
        .collect();
    if !assistant.is_empty() {
        canonical.messages.push(CanonicalMessage {
            role: Role::Assistant,
            content: assistant,
        });
    }

    let results: Vec<CanonicalContent> = params
        .tool_calls
        .iter()
        .zip(params.tool_results.iter())
        .map(|(tc, tr)| CanonicalContent::ToolResult {
            // Gemini matches a functionResponse to its call by name, not id.
            tool_use_id: tc.name.clone(),
            content: tool_result_content(tr),
            is_error: tr.is_error.unwrap_or(false),
            structured_content: tr.structured_content.clone(),
            meta: tool_result_meta(tr),
        })
        .collect();
    if !results.is_empty() {
        canonical.messages.push(CanonicalMessage {
            role: Role::Tool,
            content: results,
        });
    }

    let body = gemini::build_request_body(&canonical, None);
    let value = transport::post(provider, &body, params.model, false)
        .await?
        .json()
        .await?;
    let parsed = gemini::parse_response(&value, params.model);
    Ok(canonical_bridge::to_ai_response(
        "gemini",
        params.model,
        request_id,
        start,
        &parsed,
    ))
}

fn tool_result_content(result: &CallToolResult) -> Vec<CanonicalContent> {
    result
        .content
        .iter()
        .filter_map(|c| match &c.raw {
            RawContent::Text(text) => Some(CanonicalContent::Text(text.text.clone())),
            _ => None,
        })
        .collect()
}

fn tool_result_meta(result: &CallToolResult) -> Option<Value> {
    result
        .meta
        .as_ref()
        .and_then(|m| serde_json::to_value(m).ok())
}
