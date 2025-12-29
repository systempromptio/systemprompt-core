use anyhow::{anyhow, Result};
use reqwest::Client;
use std::time::Instant;
use uuid::Uuid;

use crate::models::ai::{AiMessage, AiResponse, SamplingParams};
use crate::models::providers::gemini::{GeminiPart, GeminiRequest, GeminiResponse};

use super::constants::timeout;
use super::provider::GeminiProvider;
use super::request_builders::AiResponseParams;
use super::{converters, request_builders};

pub struct SchemaGenerationParams<'a> {
    pub messages: &'a [AiMessage],
    pub response_schema: serde_json::Value,
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
}

pub fn build_client() -> Result<Client> {
    Client::builder()
        .timeout(timeout::REQUEST_TIMEOUT)
        .connect_timeout(timeout::CONNECT_TIMEOUT)
        .build()
        .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))
}

pub async fn generate(
    provider: &GeminiProvider,
    messages: &[AiMessage],
    sampling: Option<&SamplingParams>,
    max_output_tokens: u32,
    model: &str,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let contents = converters::convert_messages(messages);
    let generation_config =
        request_builders::build_generation_config(sampling, max_output_tokens, None, None);

    let request = GeminiRequest {
        contents,
        generation_config: Some(generation_config),
        safety_settings: None,
        tools: None,
        tool_config: None,
    };

    let response_text =
        request_builders::send_request(provider, &request, model, "generateContent").await?;

    let gemini_response: GeminiResponse = request_builders::parse_response(&response_text)?;
    let content = extract_content(&gemini_response)?;

    Ok(request_builders::build_ai_response(
        AiResponseParams::builder(request_id, &gemini_response, model, start, content).build(),
    ))
}

pub async fn generate_with_schema(
    provider: &GeminiProvider,
    params: SchemaGenerationParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let contents = converters::convert_messages(params.messages);
    let generation_config = request_builders::build_generation_config(
        params.sampling,
        params.max_output_tokens,
        Some(("application/json".to_string(), params.response_schema)),
        None,
    );

    let request = GeminiRequest {
        contents,
        generation_config: Some(generation_config),
        safety_settings: None,
        tools: None,
        tool_config: None,
    };

    let response_text =
        request_builders::send_request(provider, &request, params.model, "generateContent").await?;

    let gemini_response: GeminiResponse = request_builders::parse_response(&response_text)?;
    let content = extract_content(&gemini_response)?;

    Ok(request_builders::build_ai_response(
        AiResponseParams::builder(request_id, &gemini_response, params.model, start, content)
            .build(),
    ))
}

fn extract_content(gemini_response: &GeminiResponse) -> Result<String> {
    let candidate = gemini_response
        .candidates
        .first()
        .ok_or_else(|| anyhow!("No response from Gemini"))?;

    candidate.content.as_ref().map_or_else(
        || {
            let reason = candidate.finish_reason.as_deref().unwrap_or("UNKNOWN");
            Err(anyhow!(
                "Gemini returned no content. Finish reason: {reason}"
            ))
        },
        |content| {
            Ok(content
                .parts
                .iter()
                .filter_map(|part| match part {
                    GeminiPart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect())
        },
    )
}
