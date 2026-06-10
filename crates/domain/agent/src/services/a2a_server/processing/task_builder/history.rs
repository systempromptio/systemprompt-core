//! Reconstruction of the multi-turn message history: one agent/user message
//! pair per tool iteration, closed by the final synthesis message.

use serde_json::json;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::{CallToolResult, ToolCall};

use super::helpers::extract_text_from_content;
use crate::models::a2a::{Message, MessageRole, Part, TextPart};

pub(super) struct BuildHistoryParams<'a> {
    pub ctx_id: &'a ContextId,
    pub task_id: &'a TaskId,
    pub user_message: Message,
    pub tool_calls: &'a [ToolCall],
    pub tool_results: &'a [CallToolResult],
    pub final_response: &'a str,
}

pub(super) fn build_history(params: BuildHistoryParams<'_>) -> Vec<Message> {
    let BuildHistoryParams {
        ctx_id,
        task_id,
        user_message,
        tool_calls,
        tool_results,
        final_response,
    } = params;
    let mut history = Vec::new();
    history.push(user_message);

    let mut iteration = 1;
    let mut call_idx = 0;

    while call_idx < tool_calls.len() {
        let iteration_calls: Vec<_> = tool_calls
            .iter()
            .skip(call_idx)
            .take_while(|_| call_idx < tool_calls.len())
            .cloned()
            .collect();

        if iteration_calls.is_empty() {
            break;
        }

        history.push(history_message(
            ctx_id,
            task_id,
            MessageRole::Agent,
            format!("Executing {} tool(s)...", iteration_calls.len()),
            json!({
                "iteration": iteration,
                "tool_calls": iteration_calls.iter().map(|tc| {
                    json!({"id": tc.ai_tool_call_id.as_ref(), "name": tc.name})
                }).collect::<Vec<_>>()
            }),
        ));

        history.push(history_message(
            ctx_id,
            task_id,
            MessageRole::User,
            iteration_results_text(&iteration_calls, tool_results, call_idx),
            json!({
                "iteration": iteration,
                "tool_results": true
            }),
        ));

        call_idx += iteration_calls.len();
        iteration += 1;
    }

    history.push(history_message(
        ctx_id,
        task_id,
        MessageRole::Agent,
        final_response.to_owned(),
        json!({
            "iteration": iteration,
            "final_synthesis": true
        }),
    ));

    history
}

fn history_message(
    ctx_id: &ContextId,
    task_id: &TaskId,
    role: MessageRole,
    text: String,
    metadata: serde_json::Value,
) -> Message {
    Message {
        role,
        parts: vec![Part::Text(TextPart { text })],
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: ctx_id.clone(),
        metadata: Some(metadata),
        extensions: None,
        reference_task_ids: None,
    }
}

fn iteration_results_text(
    iteration_calls: &[ToolCall],
    tool_results: &[CallToolResult],
    call_idx: usize,
) -> String {
    iteration_calls
        .iter()
        .enumerate()
        .filter_map(|(idx, call)| {
            let result_idx = call_idx + idx;
            tool_results.get(result_idx).map(|r| {
                let content_text = extract_text_from_content(&r.content);
                format!("Tool '{}' result: {}", call.name, content_text)
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}
