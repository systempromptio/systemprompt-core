use crate::models::ai::{AiRequest, AiResponse};
use tracing::{error, info, warn};
use uuid::Uuid;

pub fn log_request_start(request_id: Uuid, request: &AiRequest, provider_name: &str, model: &str) {
    info!(
        request_id = %request_id,
        provider = provider_name,
        model = model,
        messages = request.messages.len(),
        user_id = %request.context.user_id().as_str(),
        trace_id = %request.context.trace_id(),
        "AI request started"
    );
}

pub fn log_request_success(response: &AiResponse) {
    info!(
        request_id = %response.request_id,
        chars = response.content.len(),
        tokens = response.tokens_used.unwrap_or(0),
        latency_ms = response.latency_ms,
        "AI request completed"
    );
}

pub fn log_request_error(
    request_id: Uuid,
    provider_name: &str,
    latency_ms: u64,
    error: &anyhow::Error,
) {
    error!(
        request_id = %request_id,
        provider = provider_name,
        latency_ms = latency_ms,
        error = %error,
        "AI request failed"
    );
}

pub fn log_tooled_request_start(
    request_id: Uuid,
    request: &AiRequest,
    provider_name: &str,
    model: &str,
) {
    let tools = request.tools.as_deref().unwrap_or(&[]);

    info!(
        request_id = %request_id,
        provider = provider_name,
        model = model,
        messages = request.messages.len(),
        tools = tools.len(),
        user_id = %request.context.user_id().as_str(),
        trace_id = %request.context.trace_id(),
        "AI tooled request started"
    );
}

pub fn log_ai_response(response: &AiResponse, tool_call_count: usize) {
    info!(
        request_id = %response.request_id,
        chars = response.content.len(),
        tool_calls = tool_call_count,
        "AI response received"
    );

    if tool_call_count > 0 || response.content.is_empty() {
        return;
    }

    warn!(
        request_id = %response.request_id,
        expected = "tool_call",
        chars = response.content.len(),
        "AI text response"
    );
}

pub fn log_tooled_response(response: &AiResponse) {
    info!(
        request_id = %response.request_id,
        chars = response.content.len(),
        tokens = response.tokens_used.unwrap_or(0),
        tool_calls = response.tool_calls.len(),
        latency_ms = response.latency_ms,
        "AI tooled response"
    );
}
