use std::sync::Arc;

use axum::response::sse::Event;
use serde_json::json;
use systemprompt_identifiers::{ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::{RequestContext, TaskMetadata};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::models::a2a::protocol::PushNotificationConfig;
use crate::models::a2a::{Message, Task, TaskState, TaskStatus};
use crate::repository::content::PushNotificationConfigRepository;
use crate::repository::context::ContextRepository;
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::errors::classify_database_error;
use crate::services::a2a_server::handlers::AgentHandlerState;

pub async fn detect_mcp_server_and_update_context(agent_name: &str, context: &mut RequestContext) {
    use systemprompt_core_mcp::services::registry::McpServerRegistry;

    let is_mcp_server = match McpServerRegistry::validate() {
        Ok(()) => McpServerRegistry::find_server(agent_name)
            .ok()
            .flatten()
            .is_some(),
        Err(_) => false,
    };

    if is_mcp_server && context.agent_name().as_str() != agent_name {
        tracing::info!(
            agent_name = %agent_name,
            context_agent = %context.agent_name().as_str(),
            "MCP server handling request from agent"
        );
    } else if !is_mcp_server && context.agent_name().as_str() != agent_name {
        tracing::warn!(
            context_agent = %context.agent_name().as_str(),
            service_agent = %agent_name,
            "Agent mismatch, using service name"
        );

        use systemprompt_identifiers::AgentName;
        context.execution.agent_name = AgentName::new(agent_name.to_string());
    }
}

pub fn resolve_task_id(message: &Message) -> TaskId {
    message
        .task_id
        .clone()
        .unwrap_or_else(|| TaskId::new(Uuid::new_v4().to_string()))
}

pub async fn validate_context(
    context_id: &ContextId,
    user_id: &UserId,
    state: &Arc<AgentHandlerState>,
    tx: &UnboundedSender<Event>,
    request_id: &Option<serde_json::Value>,
) -> Result<(), ()> {
    let context_repo = ContextRepository::new(state.db_pool.clone());

    if let Err(e) = context_repo.get_context(context_id, user_id).await {
        tracing::error!(
            context_id = %context_id,
            user_id = %user_id,
            error = %e,
            "Context validation failed"
        );

        let error_event = json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32603,
                "message": format!("Context validation failed: {e}")
            },
            "id": request_id
        });
        let _ = tx.send(Event::default().data(error_event.to_string()));
        return Err(());
    }

    tracing::info!(
        context_id = %context_id,
        user_id = %user_id,
        "Context validated"
    );

    Ok(())
}

pub async fn persist_initial_task(
    task_id: &TaskId,
    context_id: &ContextId,
    agent_name: &str,
    context: &RequestContext,
    state: &Arc<AgentHandlerState>,
    tx: &UnboundedSender<Event>,
    request_id: &Option<serde_json::Value>,
) -> Result<TaskRepository, ()> {
    let task_repo = TaskRepository::new(state.db_pool.clone());
    let metadata = TaskMetadata::new_agent_message(agent_name.to_string());

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

    if let Err(e) = task_repo
        .create_task(
            &task,
            &UserId::new(context.user_id().as_str()),
            &SessionId::new(context.session_id().as_str()),
            &TraceId::new(context.trace_id().as_str()),
            agent_name,
        )
        .await
    {
        tracing::error!(task_id = %task_id, error = %e, "Failed to persist task at start");

        let error_detail = classify_database_error(&e);
        let error_event = json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32603,
                "message": format!("Failed to create task: {error_detail}")
            },
            "id": request_id
        });
        let _ = tx.send(Event::default().data(error_event.to_string()));
        return Err(());
    }

    tracing::info!(task_id = %task_id, "Task persisted to database at stream start");

    if let Err(e) = task_repo
        .track_agent_in_context(context_id, agent_name)
        .await
    {
        tracing::warn!(context_id = %context_id, error = %e, "Failed to track agent in context");
    }

    Ok(task_repo)
}

pub async fn save_push_notification_config(
    task_id: &TaskId,
    callback_config: &Option<PushNotificationConfig>,
    state: &Arc<AgentHandlerState>,
) {
    if let Some(ref config) = callback_config {
        tracing::info!(url = %config.url, "Push notification callback registered");

        let config_repo = match PushNotificationConfigRepository::new(state.db_pool.clone()) {
            Ok(repo) => repo,
            Err(e) => {
                tracing::warn!(task_id = %task_id, error = %e, "Failed to create PushNotificationConfigRepository");
                return;
            },
        };

        if let Err(e) = config_repo.add_config(task_id, config).await {
            tracing::warn!(task_id = %task_id, error = %e, "Failed to save inline push notification config");
        } else {
            tracing::info!(task_id = %task_id, "Push notification config saved");
        }
    }
}
