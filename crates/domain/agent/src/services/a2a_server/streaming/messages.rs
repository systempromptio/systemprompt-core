use std::sync::Arc;

use axum::response::sse::Event;
use systemprompt_models::RequestContext;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::protocol::PushNotificationConfig;
use crate::models::a2a::Message;
use crate::services::a2a_server::handlers::AgentHandlerState;

use super::event_loop::{handle_stream_creation_error, process_events, ProcessEventsParams};
use super::initialization::setup_stream;
use super::types::StreamInput;
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

    let input = StreamInput {
        message,
        agent_name,
        state,
        request_id,
        context,
        callback_config,
    };

    tokio::spawn(async move {
        tracing::info!("Inside tokio::spawn - task execution started");

        let setup = match setup_stream(input, &tx).await {
            Ok(s) => s,
            Err(()) => return,
        };

        tracing::info!(agent = %setup.agent_name, "Starting message stream processing for agent");

        match setup
            .processor
            .process_message_stream(
                &setup.message,
                &setup.agent_runtime,
                &setup.agent_name,
                &setup.context,
                setup.task_id.clone(),
            )
            .await
        {
            Ok(chunk_rx) => {
                let params = ProcessEventsParams {
                    tx,
                    chunk_rx,
                    task_id: setup.task_id,
                    context_id: setup.context_id,
                    message_id: setup.message_id,
                    original_message: setup.message,
                    agent_name: setup.agent_name,
                    context: setup.context,
                    task_repo: setup.task_repo,
                    processor: setup.processor,
                    request_id: setup.request_id,
                };
                process_events(params).await;
            },
            Err(e) => {
                let webhook_context = WebhookContext::new(
                    setup.context.user_id().as_str(),
                    setup.context.auth_token().as_str(),
                );
                handle_stream_creation_error(
                    &webhook_context,
                    e,
                    &setup.task_id,
                    &setup.context_id,
                    &setup.task_repo,
                )
                .await;
            },
        }
    });

    UnboundedReceiverStream::new(rx)
}
