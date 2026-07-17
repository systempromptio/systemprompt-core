//! Gemini server-side code execution: builds a canonical request with the
//! code-execution flag set, renders it with the shared codec (which adds the
//! `codeExecution` tool), and maps the parsed reply back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use serde_json::Value;
use systemprompt_models::wire::gemini;

use crate::error::Result;
use crate::models::ai::{AiMessage, SamplingParams};
use crate::services::providers::canonical_bridge::{self, BridgeProvider, CanonicalBuild};

pub use crate::services::providers::canonical_bridge::CodeExecutionResponse;

use super::provider::GeminiProvider;
use super::transport;

pub async fn generate_with_code_execution(
    provider: &GeminiProvider,
    messages: &[AiMessage],
    sampling: Option<&SamplingParams>,
    max_output_tokens: u32,
    model: &str,
) -> Result<CodeExecutionResponse> {
    let start = Instant::now();
    let canonical = CanonicalBuild::new(BridgeProvider::Gemini, messages, model, max_output_tokens)
        .with_sampling(sampling)
        .with_code_execution(true)
        .into_request();

    let body = gemini::build_request_body(&canonical, None);
    let value: Value = transport::post(provider, &body, model, false)
        .await?
        .json()
        .await?;
    let parsed = gemini::parse_response(&value, model);
    Ok(canonical_bridge::to_code_execution(start, &parsed))
}
