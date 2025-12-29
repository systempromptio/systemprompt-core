use anyhow::{anyhow, Result};
use std::time::Instant;
use uuid::Uuid;

use crate::models::ai::{AiResponse, SamplingParams};
use crate::models::providers::gemini::{
    GeminiGenerationConfig, GeminiRequest, GeminiResponse, GeminiThinkingConfig,
    GeminiUsageMetadata,
};

use super::provider::GeminiProvider;

pub struct AiResponseParams<'a> {
    pub request_id: Uuid,
    pub gemini_response: &'a GeminiResponse,
    pub model: &'a str,
    pub start: Instant,
    pub content: String,
}

pub struct AiResponseParamsBuilder<'a> {
    request_id: Uuid,
    gemini_response: &'a GeminiResponse,
    model: &'a str,
    start: Instant,
    content: String,
}

impl<'a> AiResponseParamsBuilder<'a> {
    pub const fn new(
        request_id: Uuid,
        gemini_response: &'a GeminiResponse,
        model: &'a str,
        start: Instant,
        content: String,
    ) -> Self {
        Self {
            request_id,
            gemini_response,
            model,
            start,
            content,
        }
    }

    pub fn build(self) -> AiResponseParams<'a> {
        AiResponseParams {
            request_id: self.request_id,
            gemini_response: self.gemini_response,
            model: self.model,
            start: self.start,
            content: self.content,
        }
    }
}

impl<'a> AiResponseParams<'a> {
    pub const fn builder(
        request_id: Uuid,
        gemini_response: &'a GeminiResponse,
        model: &'a str,
        start: Instant,
        content: String,
    ) -> AiResponseParamsBuilder<'a> {
        AiResponseParamsBuilder::new(request_id, gemini_response, model, start, content)
    }
}

pub fn build_generation_config(
    sampling: Option<&SamplingParams>,
    max_output_tokens: u32,
    response_format: Option<(String, serde_json::Value)>,
    thinking_config: Option<GeminiThinkingConfig>,
) -> GeminiGenerationConfig {
    let (temperature, top_p, top_k, stop_sequences) = sampling
        .map_or((None, None, None, None), |s| {
            (s.temperature, s.top_p, s.top_k, s.stop_sequences.clone())
        });

    GeminiGenerationConfig {
        temperature,
        top_p,
        top_k,
        max_output_tokens: Some(max_output_tokens),
        stop_sequences,
        response_mime_type: response_format.as_ref().map(|(mime, _)| mime.clone()),
        response_schema: response_format.map(|(_, schema)| schema),
        response_modalities: None,
        image_config: None,
        thinking_config,
    }
}

pub fn build_url(endpoint: &str, model: &str, api_key: &str, method: &str) -> String {
    format!("{}/models/{}:{}?key={}", endpoint, model, method, api_key)
}

pub async fn send_request(
    provider: &GeminiProvider,
    request: &GeminiRequest,
    model: &str,
    method: &str,
) -> Result<String> {
    let url = build_url(&provider.endpoint, model, &provider.api_key, method);
    let response = provider.client.post(&url).json(request).send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await?;
        return Err(anyhow!("Gemini API error ({status}): {error_text}"));
    }

    Ok(response.text().await?)
}

pub fn parse_response<T: serde::de::DeserializeOwned>(response_text: &str) -> Result<T> {
    serde_json::from_str(response_text).map_err(|e| {
        anyhow!(
            "Failed to parse Gemini response: {}. Preview: {}",
            e,
            &response_text.chars().take(500).collect::<String>()
        )
    })
}

pub fn extract_token_usage(
    usage: Option<GeminiUsageMetadata>,
) -> (Option<u32>, Option<u32>, Option<u32>) {
    usage.map_or((None, None, None), |u| {
        (Some(u.total), Some(u.prompt), u.candidates)
    })
}

pub fn build_ai_response(params: AiResponseParams<'_>) -> AiResponse {
    let candidate = params.gemini_response.candidates.first();
    let (tokens_used, input_tokens, output_tokens) =
        extract_token_usage(params.gemini_response.usage_metadata);

    AiResponse {
        request_id: params.request_id,
        content: params.content,
        provider: "gemini".to_string(),
        model: params.model.to_string(),
        finish_reason: candidate.and_then(|c| c.finish_reason.clone()),
        tokens_used,
        input_tokens,
        output_tokens,
        cache_hit: false,
        cache_read_tokens: None,
        cache_creation_tokens: None,
        is_streaming: false,
        latency_ms: params.start.elapsed().as_millis() as u64,
        tool_calls: Vec::new(),
        tool_results: Vec::new(),
    }
}
