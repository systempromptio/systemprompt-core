//! Buffered Anthropic generation entry points.
//!
//! Each builds a canonical request through the bridge, renders it with the
//! shared Anthropic codec, posts it, and maps the parsed canonical reply back
//! to an [`AiResponse`]. No vendor wire shapes live here.

use std::time::Instant;

use serde_json::Value;
use systemprompt_models::wire::anthropic;
use systemprompt_models::wire::canonical::{CanonicalContent, ResponseFormat};
use uuid::Uuid;

use crate::error::Result;
use crate::models::ai::AiResponse;
use crate::models::tools::ToolCall;
use crate::services::providers::canonical_bridge::{
    self, BridgeProvider, CanonicalBuild, tools_to_canonical,
};
use crate::services::providers::{GenerationParams, SchemaGenerationParams, ToolGenerationParams};

use super::provider::AnthropicProvider;
use super::request::post_body;

const STRUCTURED_OUTPUT_TOOL: &str = "structured_output";

pub(super) async fn generate(
    provider: &AnthropicProvider,
    params: GenerationParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let canonical = CanonicalBuild::new(
        BridgeProvider::Anthropic,
        params.messages,
        params.model,
        params.max_output_tokens,
    )
    .with_sampling(params.sampling)
    .into_request();

    let body = anthropic::build_request_body(&canonical, params.model);
    let value: Value = post_body(provider, &body).await?.json().await?;
    let parsed = anthropic::parse_response(&value, params.model);
    Ok(canonical_bridge::to_ai_response(
        "anthropic",
        params.model,
        request_id,
        start,
        &parsed,
    ))
}

pub(super) async fn generate_with_tools(
    provider: &AnthropicProvider,
    params: ToolGenerationParams<'_>,
) -> Result<(AiResponse, Vec<ToolCall>)> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let canonical = CanonicalBuild::new(
        BridgeProvider::Anthropic,
        params.base.messages,
        params.base.model,
        params.base.max_output_tokens,
    )
    .with_sampling(params.base.sampling)
    .with_tools(tools_to_canonical(params.tools))
    .into_request();

    let body = anthropic::build_request_body(&canonical, params.base.model);
    let value: Value = post_body(provider, &body).await?.json().await?;
    let parsed = anthropic::parse_response(&value, params.base.model);
    let tool_calls = canonical_bridge::tool_calls(&parsed);
    let ai_response = canonical_bridge::to_ai_response(
        "anthropic",
        params.base.model,
        request_id,
        start,
        &parsed,
    );
    Ok((ai_response, tool_calls))
}

pub(super) async fn generate_with_schema(
    provider: &AnthropicProvider,
    params: SchemaGenerationParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let response_format = ResponseFormat::JsonSchema {
        name: STRUCTURED_OUTPUT_TOOL.to_owned(),
        schema: params.response_schema,
        strict: true,
    };
    let canonical = CanonicalBuild::new(
        BridgeProvider::Anthropic,
        params.base.messages,
        params.base.model,
        params.base.max_output_tokens,
    )
    .with_sampling(params.base.sampling)
    .with_response_format(Some(response_format))
    .into_request();

    let body = anthropic::build_request_body(&canonical, params.base.model);
    let value: Value = post_body(provider, &body).await?.json().await?;
    let parsed = anthropic::parse_response(&value, params.base.model);

    let mut ai_response = canonical_bridge::to_ai_response(
        "anthropic",
        params.base.model,
        request_id,
        start,
        &parsed,
    );
    // The schema arrives as the `structured_output` tool call; surface its
    // arguments as the response body, not as a tool invocation.
    ai_response.content = parsed
        .content
        .iter()
        .find_map(|block| match block {
            CanonicalContent::ToolUse { input, .. } => serde_json::to_string(input).ok(),
            _ => None,
        })
        .unwrap_or_default();
    ai_response.tool_calls = Vec::new();
    Ok(ai_response)
}
