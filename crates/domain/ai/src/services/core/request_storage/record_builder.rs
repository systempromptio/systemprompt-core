//! Builds persisted request records from canonical requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::ai::{AiRequest, AiResponse, MessageRole};
use crate::models::{AiRequestRecord, AiRequestRecordBuilder, AiRequestRecordError, RequestStatus};
use systemprompt_identifiers::{
    AiRequestId, AiToolCallId, ContextId, McpExecutionId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::RequestContext;

pub(super) struct MessageData {
    pub role: String,
    pub content: String,
    pub sequence: i32,
}

pub(super) struct ToolCallData {
    pub ai_tool_call_id: AiToolCallId,
    pub tool_name: String,
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

pub(super) fn build_record(
    params: &BuildRecordParams<'_>,
) -> Result<AiRequestRecord, AiRequestRecordError> {
    let user_id = UserId::new(params.context.user_id().as_str());

    let mut builder = AiRequestRecordBuilder::new(
        AiRequestId::new(params.response.request_id.to_string()),
        user_id,
    )
    .provider(&params.response.provider)
    .model(&params.response.model)
    .tokens(
        params.response.input_tokens.map(|t| t as i32),
        params.response.output_tokens.map(|t| t as i32),
    )
    .cache(
        params.response.cache_hit,
        params.response.cache_read_tokens.map(|t| t as i32),
        params.response.cache_creation_tokens.map(|t| t as i32),
    )
    .streaming(params.response.is_streaming)
    .cost(params.cost_microdollars)
    .latency(params.response.latency_ms as i32);

    builder = builder.max_tokens(params.request.max_output_tokens());

    let session_id_str = params.context.session_id().as_str();
    if !session_id_str.is_empty() {
        builder = builder.session_id(SessionId::new(session_id_str));
    }

    if let Some(task_id) = params.context.task_id() {
        builder = builder.task_id(TaskId::new(task_id.as_str()));
    }

    let context_id_str = params.context.context_id().as_str();
    if !context_id_str.is_empty() {
        builder = builder.context_id(ContextId::new(context_id_str));
    }

    let trace_id_str = params.context.trace_id().as_str();
    if trace_id_str.is_empty() {
        tracing::warn!(
            request_id = %params.response.request_id,
            "ai_requests.trace_id missing: RequestContext.trace_id is empty — \
             downstream trace correlation (trace list status, ai_requests count) will break"
        );
    } else {
        builder = builder.trace_id(TraceId::new(trace_id_str));
    }

    if let Some(mcp_execution_id) = params.context.mcp_execution_id() {
        builder = builder.mcp_execution_id(McpExecutionId::new(mcp_execution_id.as_str()));
    }

    builder = match params.status {
        RequestStatus::Completed => builder.completed(),
        RequestStatus::Failed => {
            let error_text = params.error_message.unwrap_or("Unknown error");
            builder.failed(error_text)
        },
        RequestStatus::Pending => builder,
    };

    builder.build()
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
        .map(|(i, tool_call)| ToolCallData {
            ai_tool_call_id: tool_call.ai_tool_call_id.clone(),
            tool_name: tool_call.name.clone(),
            tool_input: serde_json::to_string(&tool_call.arguments).unwrap_or_else(|e| {
                tracing::warn!(
                    error = %e,
                    tool_name = %tool_call.name,
                    "Failed to serialize tool call arguments; storing empty object"
                );
                "{}".to_owned()
            }),
            sequence: i as i32,
        })
        .collect()
}
