//! Task-completion webhook broadcast for MCP-driven tasks.

use crate::repository::task::TaskRepository;
use rmcp::ErrorData as McpError;
use systemprompt_database::DbPool;
use systemprompt_identifiers::TaskId;
use systemprompt_models::Config;

/// Trigger a `task_completed` webhook broadcast; failures are logged but never
/// returned.
///
/// # Errors
/// Currently always returns `Ok(())` — broadcast errors are swallowed and
/// logged.
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
    let task_repo = TaskRepository::new(db_pool).map_err(|e| {
        McpError::internal_error(format!("Failed to create task repository: {e}"), None)
    })?;

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
