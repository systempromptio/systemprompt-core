use anyhow::{anyhow, Result};
use futures::stream::StreamExt;
use futures::Stream;
use std::pin::Pin;

use crate::models::ai::{AiMessage, SamplingParams};
use crate::models::providers::gemini::{GeminiPart, GeminiRequest, GeminiResponse};

use super::provider::GeminiProvider;
use super::{converters, request_builders};

pub async fn generate_stream(
    provider: &GeminiProvider,
    messages: &[AiMessage],
    sampling: Option<&SamplingParams>,
    max_output_tokens: u32,
    model: &str,
) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
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

    let url = request_builders::build_url(
        &provider.endpoint,
        model,
        &provider.api_key,
        "streamGenerateContent",
    );

    let response = provider.client.post(&url).json(&request).send().await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("Gemini streaming API error: {error_text}"));
    }

    let byte_stream = response.bytes_stream();

    let text_stream = byte_stream
        .map(|result| {
            result
                .map_err(|e| anyhow!("Stream error: {e}"))
                .map(|b| parse_stream_chunk(&b))
        })
        .filter(|result| {
            futures::future::ready(result.as_ref().map(|s| !s.is_empty()).unwrap_or(true))
        });

    Ok(Box::pin(text_stream))
}

fn parse_stream_chunk(bytes: &bytes::Bytes) -> String {
    let text = String::from_utf8_lossy(bytes);
    let cleaned = text
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();

    if let Some(content) = try_parse_array_format(cleaned) {
        return content;
    }

    try_parse_chunked_format(cleaned).unwrap_or_else(|| {
        if !cleaned.is_empty() {
            tracing::debug!(chunk = %cleaned, "Could not parse Gemini stream chunk");
        }
        String::new()
    })
}

fn try_parse_array_format(cleaned: &str) -> Option<String> {
    let responses: Vec<GeminiResponse> = serde_json::from_str(&format!("[{cleaned}]")).ok()?;
    extract_text_from_responses(&responses)
}

fn try_parse_chunked_format(cleaned: &str) -> Option<String> {
    for chunk in cleaned.split("\n,\n") {
        let trimmed = chunk.trim().trim_start_matches(',').trim();
        if trimmed.is_empty() || !trimmed.starts_with('{') {
            continue;
        }

        if let Ok(response) = serde_json::from_str::<GeminiResponse>(trimmed) {
            if let Some(content) = extract_text_from_responses(&[response]) {
                return Some(content);
            }
        }
    }
    None
}

fn extract_text_from_responses(responses: &[GeminiResponse]) -> Option<String> {
    for response in responses {
        let candidate = response.candidates.first()?;
        let candidate_content = candidate.content.as_ref()?;
        let content: String = candidate_content
            .parts
            .iter()
            .filter_map(|part| match part {
                GeminiPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect();

        if !content.is_empty() {
            return Some(content);
        }
    }
    None
}
