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
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::TaskMetadata;
use systemprompt_models::execution::context::RequestContext;

#[derive(Debug)]
pub struct TaskResult {
    pub task_id: TaskId,
    pub is_owner: bool,
}

pub async fn ensure_task_exists(
    db_pool: &DbPool,
    request_context: &mut RequestContext,
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

    let context_repo = ContextRepository::new(db_pool).map_err(|e| {
        McpError::internal_error(format!("Failed to create context repository: {e}"), None)
    })?;

    let context_id = resolve_context_id(&context_repo, request_context).await?;

    create_mcp_task(CreateMcpTaskParams {
        db_pool,
        request_context,
        context_id: &context_id,
        tool_name,
        mcp_server_name,
    })
    .await
}

async fn resolve_context_id(
    context_repo: &ContextRepository,
    request_context: &mut RequestContext,
) -> Result<ContextId, McpError> {
    if request_context.context_id().as_str().is_empty() {
        find_or_create_session_context(context_repo, request_context).await
    } else {
        validate_or_replace_context(context_repo, request_context).await
    }
}

async fn find_or_create_session_context(
    context_repo: &ContextRepository,
    request_context: &mut RequestContext,
) -> Result<ContextId, McpError> {
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
        return Ok(existing.context_id);
    }

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
    Ok(new_context_id)
}

async fn validate_or_replace_context(
    context_repo: &ContextRepository,
    request_context: &mut RequestContext,
) -> Result<ContextId, McpError> {
    let old_context_id = request_context.context_id().clone();
    match context_repo
        .validate_context_ownership(&old_context_id, request_context.user_id())
        .await
    {
        Ok(()) => Ok(old_context_id),
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
            Ok(new_context_id)
        },
    }
}

struct CreateMcpTaskParams<'a> {
    db_pool: &'a DbPool,
    request_context: &'a mut RequestContext,
    context_id: &'a ContextId,
    tool_name: &'a str,
    mcp_server_name: &'a str,
}

async fn create_mcp_task(params: CreateMcpTaskParams<'_>) -> Result<TaskResult, McpError> {
    let CreateMcpTaskParams {
        db_pool,
        request_context,
        context_id,
        tool_name,
        mcp_server_name,
    } = params;

    let task_repo = TaskRepository::new(db_pool).map_err(|e| {
        McpError::internal_error(format!("Failed to create task repository: {e}"), None)
    })?;

    let task_id = TaskId::generate();

    let agent_name = request_context.agent_name().to_string();

    let metadata = TaskMetadata::new_mcp_execution(
        agent_name.clone(),
        tool_name.to_owned(),
        mcp_server_name.to_owned(),
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
