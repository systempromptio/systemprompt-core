//! Streaming A2A message handling over SSE. A message queued as an agent
//! run answers with the `Submitted` task and then follows the run's status
//! and artifact updates until it is terminal ([`super::run_stream`]); a
//! request that cannot stream is answered with one JSON-RPC error frame and
//! closes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::sse::Event;
use std::sync::Arc;
use systemprompt_models::RequestContext;
use tokio_stream::wrappers::ReceiverStream;

use super::validation::validate_message_context;
use crate::models::a2a::{StreamResponse, Task};
use crate::models::a2a::jsonrpc::NumberOrString;
use crate::services::a2a_server::errors::{JsonRpcErrorBuilder, JsonRpcErrorResponse};
use crate::services::a2a_server::handlers::state::AgentHandlerState;
use crate::services::a2a_server::streaming::{
    CreateSseStreamParams, StreamRejected, create_sse_stream, jsonrpc_error_event,
    stream_response_event,
};

pub(super) async fn handle_streaming_request(
    request: crate::models::a2a::A2aRequestParams,
    state: Arc<AgentHandlerState>,
    request_id: NumberOrString,
    context: RequestContext,
) -> Result<
    impl futures::stream::Stream<Item = Result<Event, std::convert::Infallible>> + Send,
    StreamRejected,
> {
    use crate::models::a2a::A2aRequestParams;
    use futures::StreamExt;

    let request_type = match &request {
        A2aRequestParams::SendStreamingMessage(_) => "SendStreamingMessage",
        A2aRequestParams::SendMessage(_) => "SendMessage",
        A2aRequestParams::GetTask(_) => "GetTask",
        A2aRequestParams::CancelTask(_) => "CancelTask",
        _ => "Other",
    };
    tracing::info!(request_type = %request_type, "handle_streaming_request called");

    let config = state.config.read().await;
    let agent_name = systemprompt_identifiers::AgentName::new(config.name.clone());
    drop(config);

    if let A2aRequestParams::SendStreamingMessage(params) = request {
        tracing::info!("Matched SendStreamingMessage, calling create_sse_stream");

        if let Err(err) = validate_message_context(
            &params.message,
            context.user_id(),
            &state.agent_state.repositories().contexts,
        )
        .await
        {
            tracing::error!(error = %err, "Context validation failed for streaming request");
            let error =
                JsonRpcErrorBuilder::invalid_params().with_metadata("detail", err.to_string());
            return Ok(error_stream(error, &request_id).map(Ok));
        }

        let launched =
            super::agent_run::launch_if_configured(&params.message, &agent_name, &state, &context)
                .await;
        match launched {
            Ok(Some(task)) => {
                return Ok(run_stream(task, &state, request_id).await.map(Ok));
            },
            Ok(None) => {},
            Err(failure) => {
                let frame = jsonrpc_failure_event(&failure.into_jsonrpc(&request_id));
                return Ok(single_event_stream(frame).map(Ok));
            },
        }

        Ok(create_sse_stream(CreateSseStreamParams {
            message: params.message,
            agent_name,
            state,
            request_id,
            context,
        })
        .await?
        .map(Ok))
    } else {
        tracing::warn!("Request type not SendStreamingMessage, returning error stream");
        let error = JsonRpcErrorBuilder::method_not_found().with_metadata(
            "detail",
            "Only SendStreamingMessage requests are supported for streaming",
        );
        Ok(error_stream(error, &request_id).map(Ok))
    }
}

async fn run_stream(
    task: Task,
    state: &AgentHandlerState,
    request_id: NumberOrString,
) -> ReceiverStream<Event> {
    let Some(launcher) = state.agent_state.agent_run_launcher().cloned() else {
        let frame = stream_response_event(StreamResponse::Task(task), &request_id);
        return single_event_stream(frame);
    };
    let run_id = match launcher.run_for_task(&task.id).await {
        Ok(run_id) => run_id,
        Err(e) => {
            tracing::warn!(task_id = %task.id, error = %e, "Agent run lookup failed for stream");
            None
        },
    };
    super::run_stream::follow_run(task, launcher, run_id, request_id)
}

fn error_stream(error: JsonRpcErrorBuilder, request_id: &NumberOrString) -> ReceiverStream<Event> {
    single_event_stream(jsonrpc_error_event(error, request_id))
}

fn jsonrpc_failure_event(response: &JsonRpcErrorResponse) -> Event {
    match serde_json::to_string(response) {
        Ok(data) => Event::default().data(data),
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialise A2A error frame");
            jsonrpc_error_event(JsonRpcErrorBuilder::internal_error(), &response.id)
        },
    }
}

fn single_event_stream(event: Event) -> ReceiverStream<Event> {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    if let Err(e) = tx.try_send(event) {
        tracing::warn!(error = %e, "Failed to send error event to SSE client - client may have disconnected");
    }
    ReceiverStream::new(rx)
}
