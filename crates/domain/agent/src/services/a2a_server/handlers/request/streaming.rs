//! Streaming A2A message handling over SSE.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::sse::Event;
use serde_json::json;
use std::sync::Arc;
use systemprompt_models::RequestContext;
use systemprompt_models::net::validate_outbound_url;
use tokio_stream::wrappers::ReceiverStream;

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

        if let Err(err) = validate_message_context(
            &params.message,
            Some(context.user_id()),
            &state.agent_state.repositories().contexts,
        )
        .await
        {
            tracing::error!(error = %err, "Context validation failed for streaming request");
            return Ok(invalid_params_stream(&request_id, &err).map(Ok));
        }

        let callback_config = params
            .configuration
            .as_ref()
            .and_then(|c| c.push_notification_config.clone());

        if let Some(err) = callback_config
            .as_ref()
            .and_then(|c| validate_outbound_url(&c.url).err())
        {
            tracing::warn!(error = %err, "Rejected push notification config url on streaming request");
            return Ok(invalid_params_stream(
                &request_id,
                &format!("Invalid push notification config url: {err}"),
            )
            .map(Ok));
        }

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

fn invalid_params_stream(request_id: &NumberOrString, data: &str) -> ReceiverStream<Event> {
    let error_event = json!({
        "jsonrpc": "2.0",
        "error": {
            "code": -32602,
            "message": "Invalid params",
            "data": data
        },
        "id": request_id
    });

    let (tx, rx) = tokio::sync::mpsc::channel(1024);
    if let Err(e) = tx.try_send(Event::default().data(error_event.to_string())) {
        tracing::warn!(error = %e, "Failed to send error event to SSE client - client may have disconnected");
    }
    ReceiverStream::new(rx)
}
