//! Maps a [`CanonicalResponse`] from the shared codec onto the agent's
//! provider-neutral response types ([`AiResponse`], [`SearchGroundedResponse`],
//! [`CodeExecutionResponse`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use systemprompt_models::wire::canonical::{CanonicalContent, CanonicalResponse};
use uuid::Uuid;

use crate::models::ai::{AiResponse, SearchGroundedResponse, WebSource};
use crate::models::tools::ToolCall;
use systemprompt_identifiers::AiToolCallId;

#[derive(Debug, Clone)]
pub struct CodeExecutionResponse {
    pub generated_code: String,
    pub execution_output: String,
    pub success: bool,
    pub error: Option<String>,
    pub latency_ms: u64,
}

const GEMINI_CODE_OUTCOME_OK: &str = "OUTCOME_OK";

#[must_use]
pub fn text_content(response: &CanonicalResponse) -> String {
    let mut out = String::new();
    for part in &response.content {
        if let CanonicalContent::Text(t) = part {
            out.push_str(t);
        }
    }
    out
}

#[must_use]
pub fn tool_calls(response: &CanonicalResponse) -> Vec<ToolCall> {
    response
        .content
        .iter()
        .filter_map(|part| match part {
            CanonicalContent::ToolUse {
                id, name, input, ..
            } => Some(ToolCall {
                ai_tool_call_id: AiToolCallId::new(id.clone()),
                name: name.clone(),
                arguments: input.clone(),
            }),
            _ => None,
        })
        .collect()
}

#[must_use]
pub fn to_ai_response(
    provider: &str,
    model: &str,
    request_id: Uuid,
    start: Instant,
    response: &CanonicalResponse,
) -> AiResponse {
    let usage = &response.usage;
    let cache_read = (usage.cache_read_tokens > 0).then_some(usage.cache_read_tokens);
    let cache_creation = (usage.cache_creation_tokens > 0).then_some(usage.cache_creation_tokens);
    AiResponse {
        request_id,
        content: text_content(response),
        provider: provider.to_owned(),
        model: model.to_owned(),
        finish_reason: response.raw_finish_reason.clone(),
        tokens_used: Some(usage.input_tokens + usage.output_tokens),
        input_tokens: Some(usage.input_tokens),
        output_tokens: Some(usage.output_tokens),
        cache_hit: usage.cache_read_tokens > 0,
        cache_read_tokens: cache_read,
        cache_creation_tokens: cache_creation,
        is_streaming: false,
        latency_ms: start.elapsed().as_millis() as u64,
        tool_calls: tool_calls(response),
        tool_results: Vec::new(),
    }
}

#[must_use]
pub fn to_search_grounded(start: Instant, response: &CanonicalResponse) -> SearchGroundedResponse {
    let grounding = response.grounding.clone().unwrap_or_default();
    let sources: Vec<WebSource> = grounding
        .sources
        .iter()
        .map(|s| WebSource {
            title: s.title.clone().unwrap_or_default(),
            uri: s.uri.clone(),
            relevance: s.relevance.unwrap_or(0.0),
        })
        .collect();
    let confidence_scores = grounding
        .sources
        .iter()
        .map(|s| s.relevance.unwrap_or(0.0))
        .collect();
    SearchGroundedResponse {
        content: text_content(response),
        sources,
        confidence_scores,
        web_search_queries: grounding.queries,
        url_context_metadata: None,
        tokens_used: Some(response.usage.input_tokens + response.usage.output_tokens),
        latency_ms: start.elapsed().as_millis() as u64,
        finish_reason: response.raw_finish_reason.clone(),
        safety_ratings: None,
    }
}

#[must_use]
pub fn to_code_execution(start: Instant, response: &CanonicalResponse) -> CodeExecutionResponse {
    let output = response.code_execution.clone().unwrap_or_default();
    let success = output.outcome.as_deref() == Some(GEMINI_CODE_OUTCOME_OK);
    let error = (!success)
        .then(|| output.outcome.clone())
        .flatten()
        .map(|o| format!("Code execution failed: {o}"));
    CodeExecutionResponse {
        generated_code: output.code,
        execution_output: output.result.unwrap_or_default(),
        success,
        error,
        latency_ms: start.elapsed().as_millis() as u64,
    }
}
