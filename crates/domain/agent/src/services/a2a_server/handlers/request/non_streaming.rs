use crate::models::a2a::Task;
use crate::services::a2a_server::handlers::state::AgentHandlerState;
use crate::services::a2a_server::processing::message::MessageProcessor;
use crate::services::a2a_server::processing::task_builder::build_canceled_task;
use systemprompt_models::RequestContext;

use super::validation::validate_message_context;

pub async fn handle_non_streaming_request(
    request: crate::models::a2a::A2aRequestParams,
    state: &AgentHandlerState,
    context: &RequestContext,
) -> Result<Task, Box<dyn std::error::Error + Send + Sync>> {
    use crate::models::a2a::*;

    let config = state.config.read().await;
    let agent_name = config.name.clone();
    drop(config);

    match request {
        A2aRequestParams::SendMessage(params) => {
            tracing::info!("Handling message/send request");

            validate_message_context(
                &params.message,
                Some(context.user_id().as_str()),
                &state.db_pool,
            )
            .await?;

            let message_processor =
                MessageProcessor::new(state.db_pool.clone(), state.ai_service.clone())?;

            message_processor
                .handle_message(params.message, &agent_name, context)
                .await
                .map_err(|e| e.into())
        },
        A2aRequestParams::SendStreamingMessage(params) => {
            tracing::info!("Handling message/stream request (fallback to non-streaming)");

            validate_message_context(
                &params.message,
                Some(context.user_id().as_str()),
                &state.db_pool,
            )
            .await?;

            let message_processor =
                MessageProcessor::new(state.db_pool.clone(), state.ai_service.clone())?;

            message_processor
                .handle_message(params.message, &agent_name, context)
                .await
                .map_err(|e| e.into())
        },
        A2aRequestParams::GetTask(params) => {
            tracing::info!(task_id = %params.id, "Handling tasks/get request");

            use crate::repository::task::TaskRepository;
            let task_repo = TaskRepository::new(state.db_pool.clone());

            match task_repo.get_task_by_str(&params.id).await {
                Ok(Some(task)) => Ok(task),
                Ok(None) => Err(format!("Task not found: {}", params.id).into()),
                Err(e) => Err(format!("Failed to retrieve task: {e}").into()),
            }
        },
        A2aRequestParams::CancelTask(params) => {
            tracing::info!(task_id = %params.id, "Handling tasks/cancel request");

            use crate::repository::task::TaskRepository;
            let task_repo = TaskRepository::new(state.db_pool.clone());

            match task_repo.get_task_by_str(&params.id).await {
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
