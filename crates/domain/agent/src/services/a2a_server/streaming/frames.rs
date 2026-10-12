//! SSE frame construction for A2A streams.
//!
//! Every frame is a JSON-RPC response carrying the originating request `id`:
//! a `StreamResponse` result for task, status and artifact updates, or a
//! JSON-RPC error whose `data` holds the `ErrorInfo`. A stream ends by
//! closing, never by an in-band marker.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::sse::Event;
use tokio::sync::mpsc::Sender;

use crate::models::a2a::StreamResponse;
use crate::models::a2a::jsonrpc::{JSON_RPC_VERSION_2_0, JsonRpcResponse, NumberOrString};
use crate::services::a2a_server::errors::JsonRpcErrorBuilder;

const SERIALISATION_FAILURE_FRAME: &str = concat!(
    r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Internal error","data":"#,
    r#"[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"INTERNAL","#,
    r#""domain":"a2a-protocol.org"}]},"id":null}"#
);

pub fn stream_response_frame(
    response: StreamResponse,
    request_id: &NumberOrString,
) -> JsonRpcResponse<StreamResponse> {
    JsonRpcResponse {
        jsonrpc: JSON_RPC_VERSION_2_0.to_owned(),
        result: Some(response),
        error: None,
        id: request_id.clone(),
    }
}

pub fn stream_response_event(response: StreamResponse, request_id: &NumberOrString) -> Event {
    sse_event(&stream_response_frame(response, request_id))
}

pub fn jsonrpc_error_event(error: JsonRpcErrorBuilder, request_id: &NumberOrString) -> Event {
    sse_event(&error.build(request_id))
}

pub(super) async fn send_stream_response(
    tx: &Sender<Event>,
    response: StreamResponse,
    request_id: &NumberOrString,
) {
    if tx
        .send(stream_response_event(response, request_id))
        .await
        .is_err()
    {
        tracing::trace!("Failed to send stream frame, channel closed");
    }
}

fn sse_event<T: serde::Serialize>(frame: &T) -> Event {
    match serde_json::to_string(frame) {
        Ok(data) => Event::default().data(data),
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialise A2A stream frame");
            Event::default().data(SERIALISATION_FAILURE_FRAME)
        },
    }
}
