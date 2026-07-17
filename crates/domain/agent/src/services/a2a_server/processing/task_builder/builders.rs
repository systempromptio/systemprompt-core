//! Concrete [`Task`] builders for each completion path.
//!
//! Thin constructors over [`TaskBuilder`] (completed, canceled, mock,
//! submitted) plus [`build_multiturn_task`], which reconstructs the full
//! message history and per-tool artifacts for a multi-turn agentic run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::TaskBuilder;
use super::helpers::content_to_json;
use super::history::{BuildHistoryParams, build_history};
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
        .with_response_text("Task was canceled.".to_owned())
        .build()
}

pub fn build_mock_task(task_id: TaskId) -> Task {
    let mock_context_id = ContextId::generate();
    TaskBuilder::new(mock_context_id)
        .with_task_id(task_id)
        .with_state(TaskState::Completed)
        .with_response_text("Task completed successfully.".to_owned())
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
        metadata: Some(TaskMetadata::new_agent_message(agent_name.to_owned())),
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
            TaskMetadata::new_agent_message(agent_names::SYSTEM.to_owned())
                .with_extension("total_iterations".to_owned(), json!(total_iterations))
                .with_extension("total_tools_called".to_owned(), json!(tool_calls.len())),
        ),
        created_at: Some(chrono::Utc::now()),
        last_modified: Some(chrono::Utc::now()),
    }
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
            data_map.insert("call_id".to_owned(), json!(call_id));
            data_map.insert("tool_name".to_owned(), json!(tool_name));
            data_map.insert("output".to_owned(), content_to_json(&result.content));
            data_map.insert(
                "status".to_owned(),
                json!(if is_error { "error" } else { "success" }),
            );

            Some(Artifact {
                id: parsed.artifact_id,
                title: Some(format!("tool_execution_{}", idx + 1)),
                description: Some(format!("Result from tool: {tool_name}")),
                parts: vec![Part::Data(DataPart { data: data_map })],
                extensions: vec![],
                metadata: ArtifactMetadata::new(
                    "tool_execution".to_owned(),
                    ctx_id.clone(),
                    task_id.clone(),
                )
                .with_mcp_execution_id(call_id.to_owned())
                .with_tool_name(tool_name.clone())
                .with_execution_index(idx),
            })
        })
        .collect()
}
