use std::sync::Arc;

use crate::models::a2a::{A2aRequestParams, Task};
use crate::repository::task::TaskRepository;
use crate::services::a2a_server::handlers::state::AgentHandlerState;
use crate::services::a2a_server::processing::message::MessageProcessor;
use crate::services::a2a_server::processing::task_builder::build_canceled_task;
use systemprompt_models::RequestContext;

use super::validation::validate_message_context;

pub async fn handle_non_streaming_request(
    request: A2aRequestParams,
    state: &AgentHandlerState,
    context: &RequestContext,
) -> Result<Task, Box<dyn std::error::Error + Send + Sync>> {
    let config = state.config.read().await;
    let agent_name = config.name.clone();
    drop(config);

    match request {
        A2aRequestParams::SendMessage(params) => {
            tracing::info!("Handling SendMessage request");

            validate_message_context(&params.message, Some(context.user_id()), &state.db_pool)
                .await?;

            let message_processor =
                MessageProcessor::new(&state.db_pool, Arc::clone(&state.ai_service))?;

            message_processor
                .handle_message(params.message, &agent_name, context)
                .await
                .map_err(Into::into)
        },
        A2aRequestParams::SendStreamingMessage(params) => {
            tracing::info!("Handling SendStreamingMessage request (fallback to non-streaming)");

            validate_message_context(&params.message, Some(context.user_id()), &state.db_pool)
                .await?;

            let message_processor =
                MessageProcessor::new(&state.db_pool, Arc::clone(&state.ai_service))?;

            message_processor
                .handle_message(params.message, &agent_name, context)
                .await
                .map_err(Into::into)
        },
        A2aRequestParams::GetTask(params) => {
            tracing::info!(task_id = %params.id, "Handling GetTask request");

            let task_repo = TaskRepository::new(&state.db_pool)?;
            let task_id = systemprompt_identifiers::TaskId::new(&params.id);

            match task_repo.get_task(&task_id).await {
                Ok(Some(task)) => Ok(task),
                Ok(None) => Err(format!("Task not found: {}", params.id).into()),
                Err(e) => Err(format!("Failed to retrieve task: {e}").into()),
            }
        },
        A2aRequestParams::CancelTask(params) => {
            tracing::info!(task_id = %params.id, "Handling CancelTask request");

            let task_repo = TaskRepository::new(&state.db_pool)?;
            let task_id = systemprompt_identifiers::TaskId::new(&params.id);

            match task_repo.get_task(&task_id).await {
                Ok(Some(task)) => Ok(build_canceled_task(params.id.into(), task.context_id)),
                Ok(None) => Err(format!("Task not found: {}", params.id).into()),
                Err(e) => Err(format!("Failed to look up task: {e}").into()),
            }
        },
        A2aRequestParams::SetTaskPushNotificationConfig(_)
        | A2aRequestParams::GetTaskPushNotificationConfig(_)
        | A2aRequestParams::ListTaskPushNotificationConfig(_)
        | A2aRequestParams::DeleteTaskPushNotificationConfig(_) => {
            Err("Push notification config requests should be handled before this point".into())
        },
        _ => {
            tracing::warn!(request = ?request, "Unsupported A2A request type");
            Err("Unsupported request type".into())
        },
    }
}
