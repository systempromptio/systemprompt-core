use chrono::Utc;
use serde_json::json;
use systemprompt_agent::repository::context::ContextNotificationRepository;
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_events::EventRouter;
use systemprompt_identifiers::{AgentId, ContextId, TaskId, UserId};
use systemprompt_models::{AgUiEventBuilder, CustomPayload, GenericCustomPayload};
use systemprompt_runtime::AppContext;

use super::A2aNotification;
use super::error::NotificationError;

pub(super) async fn persist_notification(
    db: systemprompt_database::DbPool,
    context: &str,
    agent: &str,
    notification: &A2aNotification,
) -> Result<i32, NotificationError> {
    let repo = ContextNotificationRepository::new(&db)?;
    let notification_data = serde_json::to_value(notification)?;
    let id = repo
        .insert(
            &ContextId::new(context),
            &AgentId::new(agent),
            &notification.method,
            &notification_data,
        )
        .await?;
    Ok(id)
}

pub(super) async fn process_notification(
    app_context: AppContext,
    notification: &A2aNotification,
) -> Result<(), NotificationError> {
    let db = app_context.db_pool();

    match notification.method.as_str() {
        "notifications/taskStatusUpdate" => {
            let task_id = notification
                .params
                .get("taskId")
                .and_then(|v| v.as_str())
                .ok_or(NotificationError::MissingField("taskId"))?;

            let status = notification
                .params
                .get("status")
                .ok_or(NotificationError::MissingField("status"))?;

            let state = status
                .get("state")
                .and_then(|v| v.as_str())
                .ok_or(NotificationError::MissingField("status.state"))?;

            let timestamp = status
                .get("timestamp")
                .and_then(systemprompt_database::parse_database_datetime)
                .unwrap_or_else(Utc::now);

            let task_repo = TaskRepository::new(db)?;
            task_repo
                .apply_notification_status(&TaskId::new(task_id), state, &timestamp)
                .await?;

            Ok(())
        },
        _ => Ok(()),
    }
}

pub(super) async fn broadcast_notification(
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

            let (agui, ctx) = EventRouter::route_agui(user_id, event).await;
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

            let (agui, ctx) = EventRouter::route_agui(user_id, event).await;
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

            let (agui, ctx) = EventRouter::route_agui(user_id, event).await;
            total_broadcasts += agui + ctx;
        },
        _ => {},
    }

    total_broadcasts
}

pub(super) async fn mark_notification_broadcasted(
    db: systemprompt_database::DbPool,
    notification_id: i32,
) -> Result<(), NotificationError> {
    let repo = ContextNotificationRepository::new(&db)?;
    repo.mark_broadcasted(notification_id).await?;
    Ok(())
}
