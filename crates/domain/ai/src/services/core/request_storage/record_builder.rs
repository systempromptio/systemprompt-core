//! Builds persisted request records from canonical requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::ai::{AiRequest, AiResponse, MessageRole};
use crate::models::{
    AiRequestRecord, AiRequestRecordBuilder, RequestKind, RequestOrigin, RequestStatus,
};
use systemprompt_identifiers::{ActorKind, AiToolCallId, McpToolName};
use systemprompt_models::RequestContext;
use systemprompt_wire::canonical::CanonicalUsage;

pub(super) struct MessageData {
    pub role: String,
    pub content: String,
    pub sequence: i32,
}

pub(super) struct ToolCallData {
    pub ai_tool_call_id: AiToolCallId,
    pub tool_name: McpToolName,
    pub tool_input: String,
    pub sequence: i32,
}

#[derive(Debug)]
pub(super) struct BuildRecordParams<'a> {
    pub request: &'a AiRequest,
    pub response: &'a AiResponse,
    pub context: &'a RequestContext,
    pub status: RequestStatus,
    pub error_message: Option<&'a str>,
    pub cost_microdollars: i64,
}

// Why: a job never produces a conversational turn. Stamping its calls as
// utility lets a dashboard exclude judge and housekeeping inference from user
// figures by `request_kind` as well as by `actor_kind`.
const fn request_kind_of(context: &RequestContext) -> RequestKind {
    match context.actor().kind {
        ActorKind::Job { .. } => RequestKind::Utility,
        _ => RequestKind::Turn,
    }
}

pub(super) fn build_record(params: &BuildRecordParams<'_>) -> AiRequestRecord {
    let user_id = params.context.user_id().clone();

    let mut builder = AiRequestRecordBuilder::new(
        params.response.request_id.clone(),
        user_id,
        params.context.context_id().clone(),
        RequestOrigin::INTERNAL,
    )
    .actor(params.context.actor().clone())
    .request_kind(request_kind_of(params.context))
    .provider(&params.response.provider)
    .model(&params.response.model)
    .usage(response_usage(params.response))
    .streaming(params.response.is_streaming)
    .cost(params.cost_microdollars)
    .latency(params.response.latency_ms as i32);

    builder = builder.max_tokens(params.request.max_output_tokens());

    let session_id = params.context.session_id();
    if !session_id.as_str().is_empty() {
        builder = builder.session_id(session_id.clone());
    }

    if let Some(task_id) = params.context.task_id() {
        builder = builder.task_id(task_id.clone());
    }

    let trace_id = params.context.trace_id();
    if trace_id.as_str().is_empty() {
        tracing::warn!(
            request_id = %params.response.request_id,
            "RequestContext.trace_id is empty; trace correlation will be incomplete"
        );
    } else {
        builder = builder.trace_id(trace_id.clone());
    }

    if let Some(mcp_execution_id) = params.context.mcp_execution_id() {
        builder = builder.mcp_execution_id(mcp_execution_id.clone());
    }

    builder = match params.status {
        RequestStatus::Completed => builder.completed(),
        RequestStatus::Failed => {
            let error_text = params.error_message.unwrap_or("Unknown error");
            builder.failed(error_text)
        },
        RequestStatus::Rejected => builder.rejected(),
        RequestStatus::Pending => builder,
    };

    builder.build()
}

fn response_usage(response: &AiResponse) -> Option<CanonicalUsage> {
    let reported = [
        response.input_tokens,
        response.output_tokens,
        response.cache_read_tokens,
        response.cache_creation_tokens,
        response.reasoning_tokens,
        response.tokens_used,
    ];
    if !reported.iter().any(Option::is_some) {
        return None;
    }
    Some(CanonicalUsage {
        input_tokens: response.input_tokens.unwrap_or(0),
        output_tokens: response.output_tokens.unwrap_or(0),
        cache_read_tokens: response.cache_read_tokens.unwrap_or(0),
        cache_creation_tokens: response.cache_creation_tokens.unwrap_or(0),
        reasoning_tokens: response.reasoning_tokens.unwrap_or(0),
        total_tokens: response.tokens_used.unwrap_or(0),
    })
}

pub(super) fn extract_messages(
    request: &AiRequest,
    response: &AiResponse,
    status: RequestStatus,
) -> Vec<MessageData> {
    let mut messages = Vec::new();
    let mut sequence = 0;

    for message in &request.messages {
        let role = message_role_to_str(message.role);

        messages.push(MessageData {
            role: role.to_owned(),
            content: message.content.clone(),
            sequence,
        });
        sequence += 1;
    }

    if status == RequestStatus::Completed && !response.content.is_empty() {
        messages.push(MessageData {
            role: message_role_to_str(MessageRole::Assistant).to_owned(),
            content: response.content.clone(),
            sequence,
        });
    }

    messages
}

const fn message_role_to_str(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
    }
}

pub(super) fn extract_tool_calls(response: &AiResponse) -> Vec<ToolCallData> {
    response
        .tool_calls
        .iter()
        .enumerate()
        .filter_map(|(i, tool_call)| {
            let tool_name = match McpToolName::try_new(tool_call.name.as_str()) {
                Ok(name) => name,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        request_id = %response.request_id,
                        sequence = i,
                        "Provider tool call has no usable tool name; not recording it"
                    );
                    return None;
                },
            };
            let tool_input = serde_json::to_string(&tool_call.arguments).unwrap_or_else(|e| {
                tracing::warn!(
                    error = %e,
                    tool_name = %tool_name,
                    "Failed to serialize tool call arguments; storing empty object"
                );
                "{}".to_owned()
            });
            Some(ToolCallData {
                ai_tool_call_id: tool_call.ai_tool_call_id.clone(),
                tool_name,
                tool_input,
                sequence: i as i32,
            })
        })
        .collect()
}
