use crate::models::a2a::{Artifact, Message, Part, Task, TaskState, TaskStatus, TextPart};
use crate::repository::context::ContextRepository;
use crate::repository::task::TaskRepository;
use crate::services::MessageService;
use rmcp::ErrorData as McpError;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, MessageId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::{Config, TaskMetadata};

#[derive(Debug)]
pub struct TaskResult {
    pub task_id: TaskId,
    pub is_owner: bool,
}

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
    let context_repo = ContextRepository::new(db_pool.clone());

    let context_id = if context_id.is_empty() {
        match context_repo
            .find_by_session_id(request_context.session_id())
            .await
        {
            Ok(Some(existing)) => {
                tracing::debug!(
                    context_id = %existing.context_id,
                    session_id = %request_context.session_id(),
                    "Reusing existing context for MCP session"
                );
                request_context.execution.context_id = existing.context_id.clone();
                existing.context_id
            },
            _ => {
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
            },
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
            }
        }
    };

    let task_repo = TaskRepository::new(db_pool.clone());

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
        kind: "task".to_string(),
    };

    task_repo
        .create_task(
            &task,
            request_context.user_id(),
            request_context.session_id(),
            request_context.trace_id(),
            &agent_name,
        )
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

pub async fn complete_task(
    db_pool: &DbPool,
    task_id: &TaskId,
    jwt_token: &str,
) -> Result<(), McpError> {
    if let Err(e) = trigger_task_completion_broadcast(db_pool, task_id, jwt_token).await {
        tracing::error!(
            task_id = %task_id.as_str(),
            error = ?e,
            "Webhook broadcast failed"
        );
    }

    Ok(())
}

async fn trigger_task_completion_broadcast(
    db_pool: &DbPool,
    task_id: &TaskId,
    jwt_token: &str,
) -> Result<(), McpError> {
    let task_repo = TaskRepository::new(db_pool.clone());

    let task_info = task_repo
        .get_task_context_info(task_id)
        .await
        .map_err(|e| {
            McpError::internal_error(format!("Failed to load task for webhook: {e}"), None)
        })?;

    if let Some(info) = task_info {
        let context_id = info.context_id;
        let user_id = info.user_id;

        let config = Config::get().map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let webhook_url = format!("{}/api/v1/webhook/broadcast", config.api_server_url);
        let webhook_payload = serde_json::json!({
            "event_type": "task_completed",
            "entity_id": task_id.as_str(),
            "context_id": context_id,
            "user_id": user_id,
        });

        tracing::debug!(
            task_id = %task_id.as_str(),
            context_id = %context_id,
            "Webhook triggering"
        );

        let client = reqwest::Client::new();
        match client
            .post(webhook_url)
            .header("Authorization", format!("Bearer {jwt_token}"))
            .json(&webhook_payload)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    tracing::debug!(
                        task_id = %task_id.as_str(),
                        "Task completed, webhook success"
                    );
                } else {
                    let status = response.status();
                    tracing::error!(
                        task_id = %task_id.as_str(),
                        status = %status,
                        "Task completed, webhook failed"
                    );
                }
            },
            Err(e) => {
                tracing::error!(
                    task_id = %task_id.as_str(),
                    error = %e,
                    "Webhook failed"
                );
            },
        }
    }

    Ok(())
}

pub async fn save_messages_for_tool_execution(
    db_pool: &DbPool,
    task_id: &TaskId,
    context_id: &ContextId,
    tool_name: &str,
    tool_result: &str,
    artifact: Option<&Artifact>,
    user_id: &UserId,
    session_id: &SessionId,
    trace_id: &TraceId,
) -> Result<(), McpError> {
    let message_service = MessageService::new(db_pool.clone());

    let user_message = Message {
        role: "user".to_string(),
        parts: vec![Part::Text(TextPart {
            text: format!("Execute tool: {tool_name}"),
        })],
        id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: context_id.clone(),
        kind: "message".to_string(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    };

    let agent_text = artifact.map_or_else(
        || format!("Tool execution completed. Result: {tool_result}"),
        |artifact| {
            format!(
                "Tool execution completed. Result: {}\n\nArtifact created: {} (type: {})",
                tool_result, artifact.id, artifact.metadata.artifact_type
            )
        },
    );

    let agent_message = Message {
        role: "agent".to_string(),
        parts: vec![Part::Text(TextPart { text: agent_text })],
        id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: context_id.clone(),
        kind: "message".to_string(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    };

    message_service
        .persist_messages(
            task_id,
            context_id,
            vec![user_message, agent_message],
            Some(user_id),
            session_id,
            trace_id,
        )
        .await
        .map_err(|e| McpError::internal_error(format!("Failed to save messages: {e}"), None))?;

    Ok(())
}
