use anyhow::{anyhow, Result};
use std::time::Instant;
use uuid::Uuid;

use crate::models::ai::AiResponse;
use crate::models::providers::openai::OpenAiResponse;

pub fn build_response(
    request_id: Uuid,
    openai_response: &OpenAiResponse,
    provider_name: &str,
    model: &str,
    start: Instant,
) -> Result<AiResponse> {
    let choice = openai_response
        .choices
        .first()
        .ok_or_else(|| anyhow!("No response from OpenAI"))?;

    let content = choice
        .message
        .content
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(String::new);

    let (tokens_used, input_tokens, output_tokens, cache_hit, cache_read_tokens) = openai_response
        .usage
        .as_ref()
        .map_or((None, None, None, false, None), |usage| {
            let cache_tokens = usage
                .prompt_tokens_details
                .as_ref()
                .and_then(|details| details.cached_tokens);
            let cache_hit = cache_tokens.is_some_and(|t| t > 0);
            (
                Some(usage.total_tokens),
                Some(usage.prompt_tokens),
                Some(usage.completion_tokens),
                cache_hit,
                cache_tokens,
            )
        });

    Ok(AiResponse {
        request_id,
        content,
        provider: provider_name.to_string(),
        model: model.to_string(),
        finish_reason: choice.finish_reason.clone(),
        tokens_used,
        input_tokens,
        output_tokens,
        cache_hit,
        cache_read_tokens,
        cache_creation_tokens: None,
        is_streaming: false,
        latency_ms: start.elapsed().as_millis() as u64,
        tool_calls: Vec::new(),
        tool_results: Vec::new(),
    })
}
