use std::sync::Arc;

use axum::response::sse::Event;
use systemprompt_models::RequestContext;
use tokio_stream::wrappers::ReceiverStream;

use crate::models::a2a::Message;
use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::protocol::PushNotificationConfig;
use crate::services::a2a_server::handlers::AgentHandlerState;
use crate::services::a2a_server::processing::message::ProcessMessageStreamParams;

use super::event_loop::{ProcessEventsParams, process_events};
use super::event_loop_lifecycle::handle_stream_creation_error;
use super::initialization::setup_stream;
use super::types::StreamInput;
use super::webhook_client::WebhookContext;

pub struct CreateSseStreamParams {
    pub message: Message,
    pub agent_name: String,
    pub state: Arc<AgentHandlerState>,
    pub request_id: NumberOrString,
    pub context: RequestContext,
    pub callback_config: Option<PushNotificationConfig>,
}

impl std::fmt::Debug for CreateSseStreamParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateSseStreamParams")
            .field("message", &self.message)
            .field("agent_name", &self.agent_name)
            .field("request_id", &self.request_id)
            .field("context", &self.context)
            .field("callback_config", &self.callback_config)
            .finish_non_exhaustive()
    }
}

/// Returned by [`create_sse_stream`] when the global stream-concurrency cap
/// is exhausted, so the streaming handler can answer with HTTP 503.
#[derive(Debug, Clone, Copy)]
pub struct StreamRejected;

pub async fn create_sse_stream(
    params: CreateSseStreamParams,
) -> Result<ReceiverStream<Event>, StreamRejected> {
    let CreateSseStreamParams {
        message,
        agent_name,
        state,
        request_id,
        context,
        callback_config,
    } = params;

    let Ok(permit) = Arc::clone(&state.stream_semaphore).try_acquire_owned() else {
        tracing::warn!(
            event = "a2a.stream.rejected",
            agent = %agent_name,
            available_permits = state.stream_semaphore.available_permits(),
            "A2A stream rejected: global concurrency cap reached"
        );
        return Err(StreamRejected);
    };

    let (tx, rx) = tokio::sync::mpsc::channel(1024);

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
        let _permit = permit;
        tracing::info!("Inside tokio::spawn - task execution started");
        run_stream(input, tx).await;
    });

    Ok(ReceiverStream::new(rx))
}

async fn run_stream(input: StreamInput, tx: tokio::sync::mpsc::Sender<Event>) {
    let Ok(setup) = setup_stream(input, &tx).await else {
        return;
    };

    tracing::info!(agent = %setup.agent_name, "Starting message stream processing for agent");

    match setup
        .processor
        .process_message_stream(ProcessMessageStreamParams {
            a2a_message: &setup.message,
            agent_runtime: &setup.agent_runtime,
            agent_name: &setup.agent_name,
            context: &setup.context,
            task_id: setup.task_id.clone(),
        })
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
                setup.context.user_id().clone(),
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
}
