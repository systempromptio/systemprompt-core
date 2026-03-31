use anyhow::{Result, anyhow};
use futures::Stream;
use futures::stream::StreamExt;
use std::pin::Pin;

use crate::models::providers::gemini::{GeminiPart, GeminiRequest, GeminiResponse};
use crate::services::providers::GenerationParams;
use systemprompt_models::ai::StreamChunk;

use super::provider::GeminiProvider;
use super::{converters, request_builders};

pub async fn generate_stream(
    provider: &GeminiProvider,
    params: GenerationParams<'_>,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
    let contents = converters::convert_messages(params.messages);
    let generation_config = request_builders::build_generation_config(
        params.sampling,
        params.max_output_tokens,
        None,
        None,
    );

    let request = GeminiRequest {
        contents,
        generation_config: Some(generation_config),
        safety_settings: None,
        tools: None,
        tool_config: None,
    };

    let url = request_builders::build_url(
        &provider.endpoint,
        params.model,
        &provider.api_key,
        "streamGenerateContent",
    );

    let response = provider.client.post(&url).json(&request).send().await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("Gemini streaming API error: {error_text}"));
    }

    let byte_stream = response.bytes_stream();

    let chunk_stream = byte_stream
        .map(|result| {
            result
                .map_err(|e| anyhow!("Stream error: {e}"))
                .map(|b| parse_stream_chunks(&b))
        })
        .flat_map(|result| match result {
            Ok(chunks) => futures::stream::iter(chunks.into_iter().map(Ok)).boxed(),
            Err(e) => futures::stream::iter(vec![Err(e)]).boxed(),
        });

    Ok(Box::pin(chunk_stream))
}

fn parse_stream_chunks(bytes: &bytes::Bytes) -> Vec<StreamChunk> {
    let text = String::from_utf8_lossy(bytes);
    let cleaned = text
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();

    if let Some(chunks) = try_parse_array_format(cleaned) {
        return chunks;
    }

    try_parse_chunked_format(cleaned).unwrap_or_else(Vec::new)
}

fn try_parse_array_format(cleaned: &str) -> Option<Vec<StreamChunk>> {
    let json_array = format!("[{cleaned}]");
    let responses: Vec<GeminiResponse> = serde_json::from_str(&json_array)
        .map_err(|e| {
            tracing::debug!(error = %e, chunk = %cleaned, "Failed to parse Gemini stream as JSON array");
            e
        })
        .ok()?;
    let chunks = extract_chunks_from_responses(&responses);
    if chunks.is_empty() {
        None
    } else {
        Some(chunks)
    }
}

fn try_parse_chunked_format(cleaned: &str) -> Option<Vec<StreamChunk>> {
    let mut chunks = Vec::new();
    for chunk in cleaned.split("\n,\n") {
        let trimmed = chunk.trim().trim_start_matches(',').trim();
        if trimmed.is_empty() || !trimmed.starts_with('{') {
            continue;
        }

        if let Ok(response) = serde_json::from_str::<GeminiResponse>(trimmed) {
            chunks.extend(extract_chunks_from_responses(&[response]));
        }
    }
    if chunks.is_empty() {
        None
    } else {
        Some(chunks)
    }
}

fn extract_chunks_from_responses(responses: &[GeminiResponse]) -> Vec<StreamChunk> {
    let mut chunks = Vec::new();
    for response in responses {
        if let Some(candidate) = response.candidates.first() {
            if let Some(candidate_content) = candidate.content.as_ref() {
                let content: String = candidate_content
                    .parts
                    .iter()
                    .filter_map(|part| match part {
                        GeminiPart::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect();

                if !content.is_empty() {
                    chunks.push(StreamChunk::Text(content));
                }
            }
        }

        if let Some(usage) = &response.usage_metadata {
            let finish_reason = response
                .candidates
                .first()
                .and_then(|c| c.finish_reason.clone());
            chunks.push(StreamChunk::Usage {
                input_tokens: Some(usage.prompt),
                output_tokens: usage.candidates,
                tokens_used: Some(usage.total),
                cache_read_tokens: None,
                cache_creation_tokens: None,
                finish_reason,
            });
        }
    }
    chunks
}
