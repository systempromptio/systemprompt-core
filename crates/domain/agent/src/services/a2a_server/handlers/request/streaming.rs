//! Streaming A2A message handling over SSE.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::sse::Event;
use serde_json::json;
use std::sync::Arc;
use systemprompt_models::RequestContext;

use super::validation::validate_message_context;
use crate::models::a2a::jsonrpc::NumberOrString;
use crate::services::a2a_server::handlers::state::AgentHandlerState;
use crate::services::a2a_server::streaming::{
    CreateSseStreamParams, StreamRejected, create_sse_stream,
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
    use tokio_stream::wrappers::ReceiverStream;

    let request_type = match &request {
        A2aRequestParams::SendStreamingMessage(_) => "SendStreamingMessage",
        A2aRequestParams::SendMessage(_) => "SendMessage",
        A2aRequestParams::GetTask(_) => "GetTask",
        A2aRequestParams::CancelTask(_) => "CancelTask",
        _ => "Other",
    };
    tracing::info!(request_type = %request_type, "handle_streaming_request called");

    let config = state.config.read().await;
    let agent_name = config.name.clone();
    drop(config);

    if let A2aRequestParams::SendStreamingMessage(params) = request {
        tracing::info!("Matched SendStreamingMessage, calling create_sse_stream");

        if let Err(err) =
            validate_message_context(&params.message, Some(context.user_id()), &state.db_pool).await
        {
            tracing::error!(error = %err, "Context validation failed for streaming request");

            let error_event = json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32602,
                    "message": "Invalid params",
                    "data": err
                },
                "id": &request_id
            });

            let (tx, rx) = tokio::sync::mpsc::channel(1024);
            if let Err(e) = tx.try_send(Event::default().data(error_event.to_string())) {
                tracing::warn!(error = %e, "Failed to send error event to SSE client - client may have disconnected");
            }
            return Ok(ReceiverStream::new(rx).map(Ok));
        }

        let callback_config = params
            .configuration
            .as_ref()
            .and_then(|c| c.push_notification_config.clone());

        Ok(create_sse_stream(CreateSseStreamParams {
            message: params.message,
            agent_name,
            state,
            request_id,
            context,
            callback_config,
        })
        .await?
        .map(Ok))
    } else {
        tracing::warn!("Request type not SendStreamingMessage, returning error stream");
        let error_event = json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32601,
                "message": "Method not found",
                "data": "Only SendStreamingMessage requests are supported for streaming"
            },
            "id": &request_id
        });

        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        if let Err(e) = tx.try_send(Event::default().data(error_event.to_string())) {
            tracing::warn!(error = %e, "Failed to send error event to SSE client - client may have disconnected");
        }
        Ok(ReceiverStream::new(rx).map(Ok))
    }
}
