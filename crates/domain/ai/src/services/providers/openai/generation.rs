//! Buffered `OpenAI` Chat Completions entry points.
//!
//! Each builds a canonical request through the bridge, renders it with the
//! shared `openai_chat` codec, posts it, and maps the parsed canonical reply
//! back to an [`AiResponse`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use serde_json::Value;
use systemprompt_models::wire::canonical::{CanonicalRequest, CanonicalResponse, ResponseFormat};
use uuid::Uuid;

use crate::error::Result;
use crate::models::ai::AiResponse;
use crate::models::tools::ToolCall;
use crate::services::providers::canonical_bridge::{
    self, BridgeProvider, CanonicalBuild, agent_response_format, tools_to_canonical,
};
use crate::services::providers::{
    GenerationParams, SchemaGenerationParams, StructuredGenerationParams, ToolGenerationParams,
};

use super::provider::OpenAiProvider;

const STRUCTURED_OUTPUT_TOOL: &str = "structured_output";

async fn post_generate(
    provider: &OpenAiProvider,
    canonical: &CanonicalRequest,
    model: &str,
) -> Result<CanonicalResponse> {
    let upstream = provider.upstream_model(model);
    let body = provider.render(canonical, upstream);
    let value: Value = provider
        .post(provider.target.wire(), body, upstream, false)
        .await?
        .json()
        .await?;
    provider.parse(&value, model)
}

pub(super) async fn generate(
    provider: &OpenAiProvider,
    params: GenerationParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let canonical = CanonicalBuild::new(
        BridgeProvider::OpenAi,
        params.messages,
        params.model,
        params.max_output_tokens,
    )
    .with_sampling(params.sampling)
    .into_request();

    let parsed = post_generate(provider, &canonical, params.model).await?;
    Ok(canonical_bridge::to_ai_response(
        "openai",
        params.model,
        request_id,
        start,
        &parsed,
    ))
}

pub(super) async fn generate_with_tools(
    provider: &OpenAiProvider,
    params: ToolGenerationParams<'_>,
) -> Result<(AiResponse, Vec<ToolCall>)> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let canonical = CanonicalBuild::new(
        BridgeProvider::OpenAi,
        params.base.messages,
        params.base.model,
        params.base.max_output_tokens,
    )
    .with_sampling(params.base.sampling)
    .with_tools(tools_to_canonical(params.tools))
    .into_request();

    let parsed = post_generate(provider, &canonical, params.base.model).await?;
    let tool_calls = canonical_bridge::tool_calls(&parsed);
    let ai_response =
        canonical_bridge::to_ai_response("openai", params.base.model, request_id, start, &parsed);
    Ok((ai_response, tool_calls))
}

pub(super) async fn generate_structured(
    provider: &OpenAiProvider,
    params: StructuredGenerationParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let canonical = CanonicalBuild::new(
        BridgeProvider::OpenAi,
        params.base.messages,
        params.base.model,
        params.base.max_output_tokens,
    )
    .with_sampling(params.base.sampling)
    .with_response_format(agent_response_format(params.response_format))
    .into_request();

    let parsed = post_generate(provider, &canonical, params.base.model).await?;
    Ok(canonical_bridge::to_ai_response(
        "openai",
        params.base.model,
        request_id,
        start,
        &parsed,
    ))
}

pub(super) async fn generate_with_schema(
    provider: &OpenAiProvider,
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
        BridgeProvider::OpenAi,
        params.base.messages,
        params.base.model,
        params.base.max_output_tokens,
    )
    .with_sampling(params.base.sampling)
    .with_response_format(Some(response_format))
    .into_request();

    let parsed = post_generate(provider, &canonical, params.base.model).await?;
    Ok(canonical_bridge::to_ai_response(
        "openai",
        params.base.model,
        request_id,
        start,
        &parsed,
    ))
}
