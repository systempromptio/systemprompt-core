//! Maps an [`AnthropicResponse`] onto the provider-neutral [`AiResponse`],
//! including token-usage and cache-hit accounting shared by every
//! non-streaming Anthropic call.

use std::time::Instant;
use uuid::Uuid;

use crate::models::ai::AiResponse;
use crate::models::providers::anthropic::AnthropicResponse;

#[derive(Clone, Copy)]
pub(super) struct ResponseContext<'a> {
    pub request_id: Uuid,
    pub model: &'a str,
    pub start: Instant,
}

pub(super) fn build_response(
    ctx: ResponseContext<'_>,
    response: &AnthropicResponse,
    content: String,
) -> AiResponse {
    let usage = &response.usage;

    AiResponse {
        request_id: ctx.request_id,
        content,
        provider: "anthropic".to_string(),
        model: ctx.model.to_string(),
        finish_reason: response.stop_reason.clone(),
        tokens_used: Some(usage.input + usage.output),
        input_tokens: Some(usage.input),
        output_tokens: Some(usage.output),
        cache_hit: usage.cache_read.is_some_and(|t| t > 0),
        cache_read_tokens: usage.cache_read,
        cache_creation_tokens: usage.cache_creation,
        is_streaming: false,
        latency_ms: ctx.start.elapsed().as_millis() as u64,
        tool_calls: Vec::new(),
        tool_results: Vec::new(),
    }
}
