//! Push-notification config endpoints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use systemprompt_agent::repository::OwnedTask;
use systemprompt_agent::repository::context::ContextNotificationRepository;
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_events::EventRouter;
use systemprompt_identifiers::{AgentId, ContextId, TaskId, UserId};
use systemprompt_models::{AgUiEventBuilder, CustomPayload, GenericCustomPayload};
use systemprompt_traits::RepositoryError;

use super::A2aNotification;
use super::error::NotificationError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskStatusUpdateParams {
    task_id: TaskId,
    status: TaskStatusUpdate,
}

#[derive(Debug, Deserialize)]
struct TaskStatusUpdate {
    state: String,
    // JSON: the timestamp arrives in any encoding `parse_database_datetime` accepts.
    #[serde(default)]
    timestamp: Option<serde_json::Value>,
}

pub(super) async fn persist_notification(
    repo: &ContextNotificationRepository,
    context: &ContextId,
    agent: &str,
    notification: &A2aNotification,
) -> Result<i32, NotificationError> {
    let notification_data = serde_json::to_value(notification)?;
    let id = repo
        .insert(
            context,
            &AgentId::new(agent),
            &notification.method,
            &notification_data,
        )
        .await?;
    Ok(id)
}

#[derive(Debug)]
pub(super) struct StatusUpdate {
    task: OwnedTask,
    state: String,
    timestamp: DateTime<Utc>,
}

pub(super) async fn resolve_status_update(
    tasks: &TaskRepository,
    context_id: &ContextId,
    user_id: &UserId,
    notification: &A2aNotification,
) -> Result<Option<StatusUpdate>, NotificationError> {
    if notification.method != "notifications/taskStatusUpdate" {
        return Ok(None);
    }

    let params = TaskStatusUpdateParams::deserialize(&notification.params)
        .map_err(NotificationError::InvalidParams)?;

    let task = tasks.owned_task(&params.task_id, user_id).await?;
    if task.context_id() != context_id {
        return Err(NotificationError::Repository(RepositoryError::not_found(
            "task",
            params.task_id.as_str(),
        )));
    }

    let timestamp = params
        .status
        .timestamp
        .as_ref()
        .and_then(systemprompt_database::parse_database_datetime)
        .unwrap_or_else(Utc::now);

    Ok(Some(StatusUpdate {
        task,
        state: params.status.state,
        timestamp,
    }))
}

pub(super) async fn apply_status_update(
    tasks: &TaskRepository,
    update: &StatusUpdate,
) -> Result<(), NotificationError> {
    tasks
        .apply_notification_status(&update.task, &update.state, &update.timestamp)
        .await?;
    Ok(())
}

pub(super) async fn broadcast_notification(
    router: &EventRouter,
    context: &str,
    user_id: &UserId,
    notification: &A2aNotification,
) -> usize {
    let mut total_broadcasts = 0;

    match notification.method.as_str() {
        "notifications/taskStatusUpdate" => {
            let event = AgUiEventBuilder::custom(CustomPayload::Generic(GenericCustomPayload {
                name: "task_status_changed".to_owned(),
                value: json!({
                    "contextId": context,
                    "taskId": notification.params.get("taskId"),
                    "status": notification.params.get("status"),
                    "task": notification.params.get("task"),
                }),
            }));

            let (agui, ctx) = router.route_agui(user_id, event).await.into_local_logged();
            total_broadcasts += agui + ctx;
        },
        "notifications/artifactCreated" => {
            let event = AgUiEventBuilder::custom(CustomPayload::Generic(GenericCustomPayload {
                name: "artifact".to_owned(),
                value: json!({
                    "artifact": notification.params.get("artifact"),
                    "taskId": notification.params.get("taskId"),
                    "contextId": context,
                }),
            }));

            let (agui, ctx) = router.route_agui(user_id, event).await.into_local_logged();
            total_broadcasts += agui + ctx;
        },
        "notifications/messageAdded" => {
            let event = AgUiEventBuilder::custom(CustomPayload::Generic(GenericCustomPayload {
                name: "message_added".to_owned(),
                value: json!({
                    "contextId": context,
                    "messageId": notification.params.get("messageId"),
                    "message": notification.params.get("message"),
                }),
            }));

            let (agui, ctx) = router.route_agui(user_id, event).await.into_local_logged();
            total_broadcasts += agui + ctx;
        },
        _ => {},
    }

    total_broadcasts
}

pub(super) async fn mark_notification_broadcasted(
    repo: &ContextNotificationRepository,
    notification_id: i32,
) -> Result<(), NotificationError> {
    repo.mark_broadcasted(notification_id).await?;
    Ok(())
}
