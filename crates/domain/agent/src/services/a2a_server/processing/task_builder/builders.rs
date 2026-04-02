use super::TaskBuilder;
use super::helpers::{content_to_json, extract_text_from_content};
use crate::models::a2a::{
    Artifact, DataPart, Message, MessageRole, Part, Task, TaskState, TaskStatus, TextPart,
};
use crate::services::mcp::parse_tool_response;
use serde_json::json;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::a2a::{ArtifactMetadata, TaskMetadata, agent_names};
use systemprompt_models::{CallToolResult, ToolCall};

pub fn build_completed_task(
    task_id: TaskId,
    context_id: ContextId,
    response_text: String,
    user_message: Message,
    artifacts: Vec<Artifact>,
) -> Task {
    TaskBuilder::new(context_id)
        .with_task_id(task_id)
        .with_state(TaskState::Completed)
        .with_response_text(response_text)
        .with_user_message(user_message)
        .with_artifacts(artifacts)
        .build()
}

pub fn build_canceled_task(task_id: TaskId, context_id: ContextId) -> Task {
    TaskBuilder::new(context_id)
        .with_task_id(task_id)
        .with_state(TaskState::Canceled)
        .with_response_text("Task was canceled.".to_string())
        .build()
}

pub fn build_mock_task(task_id: TaskId) -> Task {
    let mock_context_id = ContextId::generate();
    TaskBuilder::new(mock_context_id)
        .with_task_id(task_id)
        .with_state(TaskState::Completed)
        .with_response_text("Task completed successfully.".to_string())
        .build()
}

pub fn build_submitted_task(
    task_id: TaskId,
    context_id: ContextId,
    user_message: Message,
    agent_name: &str,
) -> Task {
    Task {
        id: task_id,
        context_id,
        status: TaskStatus {
            state: TaskState::Submitted,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        history: Some(vec![user_message]),
        artifacts: None,
        metadata: Some(TaskMetadata::new_agent_message(agent_name.to_string())),
        created_at: Some(chrono::Utc::now()),
        last_modified: Some(chrono::Utc::now()),
    }
}

#[derive(Debug)]
pub struct BuildMultiturnTaskParams {
    pub context_id: ContextId,
    pub task_id: TaskId,
    pub user_message: Message,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<CallToolResult>,
    pub final_response: String,
    pub total_iterations: usize,
}

pub fn build_multiturn_task(params: BuildMultiturnTaskParams) -> Task {
    let BuildMultiturnTaskParams {
        context_id,
        task_id,
        user_message,
        tool_calls,
        tool_results,
        final_response,
        total_iterations,
    } = params;
    let ctx_id = context_id;

    let history = build_history(BuildHistoryParams {
        ctx_id: &ctx_id,
        task_id: &task_id,
        user_message,
        tool_calls: &tool_calls,
        tool_results: &tool_results,
        final_response: &final_response,
    });

    let artifacts = build_artifacts(&ctx_id, &task_id, &tool_calls, &tool_results);

    Task {
        id: task_id.clone(),
        context_id: ctx_id.clone(),
        status: TaskStatus {
            state: TaskState::Completed,
            message: Some(Message {
                role: MessageRole::Agent,
                parts: vec![Part::Text(TextPart {
                    text: final_response,
                })],
                message_id: MessageId::generate(),
                task_id: Some(task_id),
                context_id: ctx_id,
                metadata: None,
                extensions: None,
                reference_task_ids: None,
            }),
            timestamp: Some(chrono::Utc::now()),
        },
        history: Some(history),
        artifacts: if artifacts.is_empty() {
            None
        } else {
            Some(artifacts)
        },
        metadata: Some(
            TaskMetadata::new_agent_message(agent_names::SYSTEM.to_string())
                .with_extension("total_iterations".to_string(), json!(total_iterations))
                .with_extension("total_tools_called".to_string(), json!(tool_calls.len())),
        ),
        created_at: Some(chrono::Utc::now()),
        last_modified: Some(chrono::Utc::now()),
    }
}

struct BuildHistoryParams<'a> {
    ctx_id: &'a ContextId,
    task_id: &'a TaskId,
    user_message: Message,
    tool_calls: &'a [ToolCall],
    tool_results: &'a [CallToolResult],
    final_response: &'a str,
}

fn build_history(params: BuildHistoryParams<'_>) -> Vec<Message> {
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

        history.push(Message {
            role: MessageRole::Agent,
            parts: vec![Part::Text(TextPart {
                text: format!("Executing {} tool(s)...", iteration_calls.len()),
            })],
            message_id: MessageId::generate(),
            task_id: Some(task_id.clone()),
            context_id: ctx_id.clone(),
            metadata: Some(json!({
                "iteration": iteration,
                "tool_calls": iteration_calls.iter().map(|tc| {
                    json!({"id": tc.ai_tool_call_id.as_ref(), "name": tc.name})
                }).collect::<Vec<_>>()
            })),
            extensions: None,
            reference_task_ids: None,
        });

        let results_text = iteration_calls
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
            .join("\n");

        history.push(Message {
            role: MessageRole::User,
            parts: vec![Part::Text(TextPart { text: results_text })],
            message_id: MessageId::generate(),
            task_id: Some(task_id.clone()),
            context_id: ctx_id.clone(),
            metadata: Some(json!({
                "iteration": iteration,
                "tool_results": true
            })),
            extensions: None,
            reference_task_ids: None,
        });

        call_idx += iteration_calls.len();
        iteration += 1;
    }

    history.push(Message {
        role: MessageRole::Agent,
        parts: vec![Part::Text(TextPart {
            text: final_response.to_string(),
        })],
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: ctx_id.clone(),
        metadata: Some(json!({
            "iteration": iteration,
            "final_synthesis": true
        })),
        extensions: None,
        reference_task_ids: None,
    });

    history
}

fn build_artifacts(
    ctx_id: &ContextId,
    task_id: &TaskId,
    tool_calls: &[ToolCall],
    tool_results: &[CallToolResult],
) -> Vec<Artifact> {
    tool_results
        .iter()
        .enumerate()
        .filter_map(|(idx, result)| {
            let tool_call = tool_calls.get(idx)?;
            let tool_name = &tool_call.name;
            let call_id = tool_call.ai_tool_call_id.as_ref();
            let is_error = result.is_error?;

            let structured_content = result.structured_content.as_ref()?;
            let parsed = parse_tool_response(structured_content)
                .map_err(|e| {
                    tracing::debug!(tool_name = %tool_name, error = %e, "Failed to parse tool response, skipping artifact");
                    e
                })
                .ok()?;

            let mut data_map = serde_json::Map::new();
            data_map.insert("call_id".to_string(), json!(call_id));
            data_map.insert("tool_name".to_string(), json!(tool_name));
            data_map.insert("output".to_string(), content_to_json(&result.content));
            data_map.insert(
                "status".to_string(),
                json!(if is_error { "error" } else { "success" }),
            );

            Some(Artifact {
                id: parsed.artifact_id,
                title: Some(format!("tool_execution_{}", idx + 1)),
                description: Some(format!("Result from tool: {tool_name}")),
                parts: vec![Part::Data(DataPart { data: data_map })],
                extensions: vec![],
                metadata: ArtifactMetadata::new(
                    "tool_execution".to_string(),
                    ctx_id.clone(),
                    task_id.clone(),
                )
                .with_mcp_execution_id(call_id.to_string())
                .with_tool_name(tool_name.clone())
                .with_execution_index(idx),
            })
        })
        .collect()
}
