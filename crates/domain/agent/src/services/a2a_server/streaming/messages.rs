use std::sync::Arc;

use axum::response::sse::Event;
use systemprompt_models::RequestContext;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;

use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::protocol::PushNotificationConfig;
use crate::models::a2a::Message;
use crate::services::a2a_server::handlers::AgentHandlerState;
use crate::services::a2a_server::processing::message::MessageProcessor;

use super::agent_loader::load_agent_runtime;
use super::broadcast::broadcast_task_created;
use super::event_loop::{handle_stream_creation_error, process_events, ProcessEventsParams};
use super::initialization::{
    detect_mcp_server_and_update_context, persist_initial_task, resolve_task_id,
    save_push_notification_config, validate_context,
};
use super::webhook_client::WebhookContext;

pub async fn create_sse_stream(
    message: Message,
    agent_name: String,
    state: Arc<AgentHandlerState>,
    request_id: NumberOrString,
    context: RequestContext,
    callback_config: Option<PushNotificationConfig>,
) -> UnboundedReceiverStream<Event> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    tracing::info!("create_sse_stream() called - spawning tokio task");

    tokio::spawn(async move {
        let tx = tx;
        tracing::info!("Inside tokio::spawn - task execution started");

        let mut context = context;
        detect_mcp_server_and_update_context(&agent_name, &mut context).await;

        let task_id = resolve_task_id(&message);
        let context_id = message.context_id.clone();
        let message_id = Uuid::new_v4().to_string();

        tracing::info!(task_id = %task_id, context_id = %context_id, message_id = %message_id, "Generated IDs");

        if validate_context(&context_id, context.user_id(), &state, &tx, &request_id)
            .await
            .is_err()
        {
            drop(tx);
            return;
        }

        let task_repo = match persist_initial_task(
            &task_id,
            &context_id,
            &agent_name,
            &context,
            &state,
            &tx,
            &request_id,
        )
        .await
        {
            Ok(repo) => repo,
            Err(()) => {
                drop(tx);
                return;
            },
        };

        broadcast_task_created(
            &task_id,
            &context_id,
            context.user_id().as_str(),
            &message,
            &agent_name,
            context.auth.auth_token.as_str(),
        )
        .await;

        save_push_notification_config(&task_id, &callback_config, &state).await;

        let agent_runtime =
            match load_agent_runtime(&agent_name, &task_id, &task_repo, &tx, &request_id).await {
                Ok(runtime) => runtime,
                Err(()) => {
                    drop(tx);
                    return;
                },
            };

        let processor = match MessageProcessor::new(state.db_pool.clone(), state.ai_service.clone())
        {
            Ok(p) => Arc::new(p),
            Err(e) => {
                tracing::error!(error = %e, "Failed to create MessageProcessor");
                let error_event = serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": format!("Failed to initialize message processor: {e}")
                    },
                    "id": &request_id
                });
                let _ = tx.send(Event::default().data(error_event.to_string()));
                drop(tx);
                return;
            },
        };

        tracing::info!(agent = %agent_name, "Starting message stream processing for agent");

        match processor
            .process_message_stream(
                &message,
                &agent_runtime,
                &agent_name,
                &context,
                task_id.clone(),
            )
            .await
        {
            Ok(chunk_rx) => {
                let params = ProcessEventsParams {
                    tx,
                    chunk_rx,
                    task_id,
                    context_id,
                    message_id,
                    original_message: message,
                    agent_name,
                    context,
                    task_repo,
                    processor,
                    request_id,
                };
                process_events(params).await;
            },
            Err(e) => {
                let webhook_context =
                    WebhookContext::new(context.user_id().as_str(), context.auth_token().as_str());
                handle_stream_creation_error(
                    &webhook_context,
                    e,
                    &task_id,
                    &context_id,
                    &task_repo,
                )
                .await;
            },
        }
    });

    UnboundedReceiverStream::new(rx)
}
