//! Buffered Gemini generation: plain and schema-constrained completions.
//!
//! Builds a canonical request through the bridge, renders it with the shared
//! Gemini codec, posts it, and maps the parsed reply back to an [`AiResponse`].

use std::time::Instant;

use serde_json::Value;
use systemprompt_models::wire::canonical::ResponseFormat;
use systemprompt_models::wire::gemini;
use uuid::Uuid;

use crate::error::Result;
use crate::models::ai::AiResponse;
use crate::services::providers::canonical_bridge::{self, BridgeProvider, CanonicalBuild};
use crate::services::providers::{GenerationParams, SchemaGenerationParams};

use super::provider::GeminiProvider;
use super::transport;

const STRUCTURED_OUTPUT_TOOL: &str = "structured_output";

pub(super) async fn generate(
    provider: &GeminiProvider,
    params: GenerationParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let canonical = CanonicalBuild::new(
        BridgeProvider::Gemini,
        params.messages,
        params.model,
        params.max_output_tokens,
    )
    .with_sampling(params.sampling)
    .into_request();

    let body = gemini::build_request_body(&canonical);
    let value: Value = transport::post(provider, &body, params.model, false)
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

pub(super) async fn generate_with_schema(
    provider: &GeminiProvider,
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
        BridgeProvider::Gemini,
        params.base.messages,
        params.base.model,
        params.base.max_output_tokens,
    )
    .with_sampling(params.base.sampling)
    .with_response_format(Some(response_format))
    .into_request();

    let body = gemini::build_request_body(&canonical);
    let value: Value = transport::post(provider, &body, params.base.model, false)
        .await?
        .json()
        .await?;
    let parsed = gemini::parse_response(&value, params.base.model);
    Ok(canonical_bridge::to_ai_response(
        "gemini",
        params.base.model,
        request_id,
        start,
        &parsed,
    ))
}
