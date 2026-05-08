//! Per-step helpers used by [`super::initialization::setup_stream`]:
//! context validation, initial task persistence, and push-notification config
//! storage.

use std::sync::Arc;

use axum::response::sse::Event;
use systemprompt_identifiers::{ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::TaskMetadata;
use tokio::sync::mpsc::Sender;

use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::protocol::PushNotificationConfig;
use crate::models::a2a::{Task, TaskState, TaskStatus};
use crate::repository::content::PushNotificationConfigRepository;
use crate::repository::context::ContextRepository;
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::errors::classify_database_error;
use crate::services::a2a_server::handlers::AgentHandlerState;

use super::initialization::create_jsonrpc_error_event;
use super::types::PersistTaskInput;

pub async fn validate_context(
    context_id: &ContextId,
    user_id: &UserId,
    state: &Arc<AgentHandlerState>,
    tx: &Sender<Event>,
    request_id: &NumberOrString,
) -> Result<(), ()> {
    let context_repo = ContextRepository::new(&state.db_pool).map_err(|e| {
        tracing::error!(error = %e, "Failed to create ContextRepository");
        if tx
            .try_send(create_jsonrpc_error_event(
                -32603,
                &format!("Failed to initialize context repository: {e}"),
                request_id,
            ))
            .is_err()
        {
            tracing::trace!("Failed to send error event, channel closed");
        }
    })?;

    context_repo
        .get_context(context_id, user_id)
        .await
        .map_err(|e| {
            tracing::error!(
                context_id = %context_id,
                user_id = %user_id,
                error = %e,
                "Context validation failed"
            );
            if tx
                .try_send(create_jsonrpc_error_event(
                    -32603,
                    &format!("Context validation failed: {e}"),
                    request_id,
                ))
                .is_err()
            {
                tracing::trace!("Failed to send error event, channel closed");
            }
        })?;

    tracing::info!(
        context_id = %context_id,
        user_id = %user_id,
        "Context validated"
    );

    Ok(())
}

pub async fn persist_initial_task(input: PersistTaskInput<'_>) -> Result<TaskRepository, ()> {
    let PersistTaskInput {
        task_id,
        context_id,
        agent_name,
        context,
        state,
        tx,
        request_id,
    } = input;

    let task_repo = TaskRepository::new(&state.db_pool).map_err(|e| {
        tracing::error!(error = %e, "Failed to create TaskRepository");
        if tx
            .try_send(create_jsonrpc_error_event(
                -32603,
                &format!("Failed to initialize task repository: {e}"),
                request_id,
            ))
            .is_err()
        {
            tracing::trace!("Failed to send error event, channel closed");
        }
    })?;
    let metadata = TaskMetadata::new_agent_message(agent_name.to_string());

    let task = Task {
        id: task_id.clone(),
        context_id: Some(context_id.clone()),
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
            user_id: &UserId::new(context.user_id().as_str()),
            session_id: &SessionId::new(context.session_id().as_str()),
            trace_id: &TraceId::new(context.trace_id().as_str()),
            agent_name,
        })
        .await
        .map_err(|e| {
            tracing::error!(task_id = %task_id, error = %e, "Failed to persist task at start");
            let error_detail = classify_database_error(&e);
            if tx
                .try_send(create_jsonrpc_error_event(
                    -32603,
                    &format!("Failed to create task: {error_detail}"),
                    request_id,
                ))
                .is_err()
            {
                tracing::trace!("Failed to send error event, channel closed");
            }
        })?;

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
    callback_config: Option<&PushNotificationConfig>,
    state: &Arc<AgentHandlerState>,
) {
    let Some(config) = callback_config else {
        return;
    };

    tracing::info!(url = %config.url, "Push notification callback registered");

    let config_repo = match PushNotificationConfigRepository::new(&state.db_pool) {
        Ok(repo) => repo,
        Err(e) => {
            tracing::warn!(task_id = %task_id, error = %e, "Failed to create PushNotificationConfigRepository");
            return;
        },
    };

    match config_repo.add_config(task_id, config).await {
        Ok(_) => tracing::info!(task_id = %task_id, "Push notification config saved"),
        Err(e) => {
            tracing::warn!(task_id = %task_id, error = %e, "Failed to save push notification config");
        },
    }
}
