use axum::response::sse::Event;
use serde_json::json;
use std::sync::Arc;
use systemprompt_models::RequestContext;

use super::validation::validate_message_context;
use crate::models::a2a::jsonrpc::NumberOrString;
use crate::services::a2a_server::handlers::state::AgentHandlerState;
use crate::services::a2a_server::streaming::create_sse_stream;

pub async fn handle_streaming_request(
    request: crate::models::a2a::A2aRequestParams,
    state: Arc<AgentHandlerState>,
    request_id: NumberOrString,
    context: RequestContext,
) -> impl futures::stream::Stream<Item = Result<Event, std::convert::Infallible>> + Send {
    use crate::models::a2a::*;
    use futures::StreamExt;
    use tokio_stream::wrappers::UnboundedReceiverStream;


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

    let stream = match request {
        A2aRequestParams::SendStreamingMessage(params) => {
            tracing::info!("Matched SendStreamingMessage, calling create_sse_stream");

            if let Err(err) = validate_message_context(
                &params.message,
                Some(context.user_id().as_str()),
                &state.db_pool,
            )
            .await
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

                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                if let Err(e) = tx.send(Event::default().data(error_event.to_string())) {
                    tracing::warn!(error = %e, "Failed to send error event to SSE client - client may have disconnected");
                }
                return UnboundedReceiverStream::new(rx).map(Ok);
            }

            let callback_config = params
                .configuration
                .as_ref()
                .and_then(|c| c.push_notification_config.clone());

            create_sse_stream(
                params.message,
                agent_name,
                state,
                request_id,
                context,
                callback_config,
            )
            .await
            .map(Ok)
        },
        _ => {
            tracing::warn!("Request type not SendStreamingMessage, returning error stream");
            let error_event = json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32601,
                    "message": "Method not found",
                    "data": "Only message/stream requests are supported for streaming"
                },
                "id": &request_id
            });

            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            if let Err(e) = tx.send(Event::default().data(error_event.to_string())) {
                tracing::warn!(error = %e, "Failed to send error event to SSE client - client may have disconnected");
            }
            UnboundedReceiverStream::new(rx).map(Ok)
        },
    };

    stream
}
