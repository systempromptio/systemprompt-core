use crate::models::ai::{AiRequest, AiResponse, MessageRole};
use crate::models::{AiRequestRecord, AiRequestRecordBuilder, RequestStatus};
use systemprompt_identifiers::{ContextId, McpExecutionId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::RequestContext;

pub struct MessageData {
    pub role: String,
    pub content: String,
    pub sequence: i32,
}

pub struct ToolCallData {
    pub ai_tool_call_id: String,
    pub tool_name: String,
    pub tool_input: String,
    pub sequence: i32,
}

#[derive(Debug)]
pub struct BuildRecordParams<'a> {
    pub request: &'a AiRequest,
    pub response: &'a AiResponse,
    pub context: &'a RequestContext,
    pub status: RequestStatus,
    pub error_message: Option<&'a str>,
    pub cost_cents: i32,
}

pub fn build_record(params: &BuildRecordParams<'_>) -> AiRequestRecord {
    let user_id = UserId::new(params.context.user_id().as_str());

    let mut builder = AiRequestRecordBuilder::new(params.response.request_id.to_string(), user_id)
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
        .cost(params.cost_cents)
        .latency(params.response.latency_ms as i32);

    builder = builder.max_tokens(params.request.max_output_tokens());

    if !params.context.session_id().as_str().is_empty() {
        builder = builder.session_id(SessionId::new(params.context.session_id().as_str()));
    }

    if let Some(task_id) = params.context.task_id() {
        builder = builder.task_id(TaskId::new(task_id.as_str()));
    }

    if !params.context.context_id().as_str().is_empty() {
        builder = builder.context_id(ContextId::new(params.context.context_id().as_str()));
    }

    if !params.context.trace_id().as_str().is_empty() {
        builder = builder.trace_id(TraceId::new(params.context.trace_id().as_str()));
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

    builder.build().unwrap_or_else(|_| {
        AiRequestRecordBuilder::new(
            params.response.request_id.to_string(),
            UserId::new("unknown"),
        )
        .provider("unknown")
        .model("unknown")
        .build()
        .unwrap_or_else(|_| {
            AiRequestRecord::minimal_fallback(params.response.request_id.to_string())
        })
    })
}

pub fn extract_messages(
    request: &AiRequest,
    response: &AiResponse,
    status: RequestStatus,
) -> Vec<MessageData> {
    let mut messages = Vec::new();
    let mut sequence = 0;

    for message in &request.messages {
        let role = message_role_to_str(message.role);

        messages.push(MessageData {
            role: role.to_string(),
            content: message.content.clone(),
            sequence,
        });
        sequence += 1;
    }

    if status == RequestStatus::Completed && !response.content.is_empty() {
        messages.push(MessageData {
            role: message_role_to_str(MessageRole::Assistant).to_string(),
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

pub fn extract_tool_calls(response: &AiResponse) -> Vec<ToolCallData> {
    response
        .tool_calls
        .iter()
        .enumerate()
        .map(|(i, tool_call)| ToolCallData {
            ai_tool_call_id: tool_call.ai_tool_call_id.to_string(),
            tool_name: tool_call.name.clone(),
            tool_input: serde_json::to_string(&tool_call.arguments)
                .unwrap_or_else(|_| "{}".to_string()),
            sequence: i as i32,
        })
        .collect()
}
