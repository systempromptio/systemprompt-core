//! Helpers used by the MCP bridge to ensure-or-create tasks, broadcast
//! completion, and persist tool-execution messages.

mod completion;
mod messages;

pub use completion::complete_task;
pub use messages::{SaveMessagesForToolExecutionParams, save_messages_for_tool_execution};

use crate::models::a2a::{Task, TaskState, TaskStatus};
use crate::repository::context::ContextRepository;
use crate::repository::task::TaskRepository;
use rmcp::ErrorData as McpError;
use systemprompt_database::DbPool;
use systemprompt_identifiers::TaskId;
use systemprompt_models::TaskMetadata;

/// Result of [`ensure_task_exists`] — either an existing task id or a freshly
/// created one.
#[derive(Debug)]
pub struct TaskResult {
    /// The task id this MCP execution will record against.
    pub task_id: TaskId,
    /// `true` if this caller is the owner that created the task.
    pub is_owner: bool,
}

/// Ensure that an MCP execution has a task to record against, creating one
/// (and a context, if needed) if the request context does not already carry
/// one.
///
/// # Errors
/// Returns [`McpError`] if any repository operation fails.
pub async fn ensure_task_exists(
    db_pool: &DbPool,
    request_context: &mut systemprompt_models::execution::context::RequestContext,
    tool_name: &str,
    mcp_server_name: &str,
) -> Result<TaskResult, McpError> {
    if let Some(task_id) = request_context.task_id() {
        tracing::info!(task_id = %task_id.as_str(), "Task reused from parent");
        return Ok(TaskResult {
            task_id: task_id.clone(),
            is_owner: false,
        });
    }

    let context_id = request_context.context_id();
    let context_repo = ContextRepository::new(db_pool).map_err(|e| {
        McpError::internal_error(format!("Failed to create context repository: {e}"), None)
    })?;

    let context_id = if context_id.is_empty() {
        if let Ok(Some(existing)) = context_repo
            .find_by_session_id(request_context.session_id())
            .await
        {
            tracing::debug!(
                context_id = %existing.context_id,
                session_id = %request_context.session_id(),
                "Reusing existing context for MCP session"
            );
            request_context.execution.context_id = existing.context_id.clone();
            existing.context_id
        } else {
            let new_context_id = context_repo
                .create_context(
                    request_context.user_id(),
                    Some(request_context.session_id()),
                    &format!("MCP Session: {}", request_context.session_id()),
                )
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to auto-create context for MCP session");
                    McpError::internal_error(format!("Failed to create context: {e}"), None)
                })?;

            request_context.execution.context_id = new_context_id.clone();
            tracing::info!(
                context_id = %new_context_id,
                session_id = %request_context.session_id(),
                "Auto-created context for MCP session"
            );
            new_context_id
        }
    } else {
        let old_context_id = context_id.clone();
        match context_repo
            .validate_context_ownership(&old_context_id, request_context.user_id())
            .await
        {
            Ok(()) => old_context_id,
            Err(e) => {
                tracing::warn!(
                    context_id = %old_context_id,
                    user_id = %request_context.user_id(),
                    error = %e,
                    "Context validation failed, auto-creating new context"
                );
                let new_context_id = context_repo
                    .create_context(
                        request_context.user_id(),
                        Some(request_context.session_id()),
                        &format!("MCP Session: {}", request_context.session_id()),
                    )
                    .await
                    .map_err(|e| {
                        tracing::error!(error = %e, "Failed to auto-create replacement context");
                        McpError::internal_error(format!("Failed to create context: {e}"), None)
                    })?;

                request_context.execution.context_id = new_context_id.clone();
                tracing::info!(
                    old_context_id = %old_context_id,
                    new_context_id = %new_context_id,
                    session_id = %request_context.session_id(),
                    "Auto-created replacement context for invalid context_id"
                );
                new_context_id
            },
        }
    };

    let task_repo = TaskRepository::new(db_pool).map_err(|e| {
        McpError::internal_error(format!("Failed to create task repository: {e}"), None)
    })?;

    let task_id = TaskId::generate();

    let agent_name = request_context.agent_name().to_string();

    let metadata = TaskMetadata::new_mcp_execution(
        agent_name.clone(),
        tool_name.to_string(),
        mcp_server_name.to_string(),
    );

    let task = Task {
        id: task_id.clone(),
        context_id: context_id.clone(),
        status: TaskStatus {
            state: TaskState::Submitted,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        history: None,
        artifacts: None,
        metadata: Some(metadata),
        created_at: Some(chrono::Utc::now()),
        last_modified: Some(chrono::Utc::now()),
    };

    task_repo
        .create_task(crate::repository::task::RepoCreateTaskParams {
            task: &task,
            user_id: request_context.user_id(),
            session_id: request_context.session_id(),
            trace_id: request_context.trace_id(),
            agent_name: &agent_name,
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Failed to create task: {e}"), None))?;

    request_context.execution.task_id = Some(task_id.clone());

    tracing::info!(
        task_id = %task_id.as_str(),
        tool = %tool_name,
        agent = %agent_name,
        "Task created"
    );

    Ok(TaskResult {
        task_id,
        is_owner: true,
    })
}
