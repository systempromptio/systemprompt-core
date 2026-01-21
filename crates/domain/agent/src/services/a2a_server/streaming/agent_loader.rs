use axum::response::sse::Event;
use systemprompt_identifiers::TaskId;
use tokio::sync::mpsc::UnboundedSender;

use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::AgentRuntimeInfo;
use crate::repository::task::TaskRepository;
use crate::services::registry::AgentRegistry;

use super::initialization::create_jsonrpc_error_event;

pub async fn load_agent_runtime(
    agent_name: &str,
    task_id: &TaskId,
    task_repo: &TaskRepository,
    tx: &UnboundedSender<Event>,
    request_id: &NumberOrString,
) -> Result<AgentRuntimeInfo, ()> {
    let registry = match AgentRegistry::new().await {
        Ok(r) => r,
        Err(e) => {
            let error_msg = format!("Failed to load agent registry: {}", e);
            tracing::error!(
                error = %e,
                "Failed to load agent registry - check if config files exist"
            );
            mark_task_failed_with_error(task_id, task_repo, &error_msg).await;
            if tx
                .send(create_jsonrpc_error_event(
                    -32603,
                    "Failed to load agent registry - check system logs for details",
                    request_id,
                ))
                .is_err()
            {
                tracing::trace!("Failed to send error event, channel closed");
            }
            return Err(());
        },
    };

    match registry.get_agent(agent_name).await {
        Ok(agent_config) => Ok(agent_config.into()),
        Err(e) => {
            let error_msg = format!("Failed to load agent '{}': {}", agent_name, e);
            tracing::error!(agent_name = %agent_name, error = %e, "Failed to load agent");
            mark_task_failed_with_error(task_id, task_repo, &error_msg).await;
            if tx
                .send(create_jsonrpc_error_event(
                    -32603,
                    "Agent not found",
                    request_id,
                ))
                .is_err()
            {
                tracing::trace!("Failed to send error event, channel closed");
            }
            Err(())
        },
    }
}

pub async fn mark_task_failed_with_error(
    task_id: &TaskId,
    task_repo: &TaskRepository,
    error_message: &str,
) {
    let failed_timestamp = chrono::Utc::now();
    if let Err(update_err) = task_repo
        .update_task_failed_with_error(task_id, error_message, &failed_timestamp)
        .await
    {
        tracing::error!(task_id = %task_id, error = %update_err, "Failed to update task to failed state");
    }
}
