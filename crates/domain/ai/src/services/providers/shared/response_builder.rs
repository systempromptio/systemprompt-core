use crate::models::ai::AiResponse;
use std::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default)]
pub struct TokenUsage {
    pub tokens_used: Option<u32>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub cache_hit: bool,
    pub cache_read_tokens: Option<u32>,
    pub cache_creation_tokens: Option<u32>,
}

#[derive(Debug)]
pub struct BuildResponseParams<'a> {
    pub request_id: Uuid,
    pub content: String,
    pub provider: &'a str,
    pub model: &'a str,
    pub finish_reason: Option<String>,
    pub usage: TokenUsage,
    pub start: Instant,
}

pub fn build_response(params: BuildResponseParams<'_>) -> AiResponse {
    let BuildResponseParams {
        request_id,
        content,
        provider,
        model,
        finish_reason,
        usage,
        start,
    } = params;
    AiResponse {
        request_id,
        content,
        provider: provider.to_string(),
        model: model.to_string(),
        finish_reason,
        tokens_used: usage.tokens_used,
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_hit: usage.cache_hit,
        cache_read_tokens: usage.cache_read_tokens,
        cache_creation_tokens: usage.cache_creation_tokens,
        is_streaming: false,
        latency_ms: start.elapsed().as_millis() as u64,
        tool_calls: Vec::new(),
        tool_results: Vec::new(),
    }
}
