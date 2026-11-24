//! The server-sent event helper the API's own streams use (the bridge event
//! stream).
//!
//! [`create_sse_stream`] registers a per-user connection with a broadcaster
//! from `systemprompt_events` (with a per-user cap), wraps the receiver in a
//! [`StreamWithGuard`] so the [`ConnectionGuard`] deregisters it on drop, and
//! emits keep-alive frames. The A2A, `AgUI` and context-state stream routes are
//! the `agent-http` member's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::IntoResponse;
use axum::response::sse::Sse;
use std::convert::Infallible;
use std::sync::Arc;
use systemprompt_events::{
    Broadcaster, ConnectionGuard, GenericBroadcaster, ToSse, standard_keep_alive,
};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::kernel::ApiHttpError;

#[derive(Debug)]
pub struct StreamWithGuard<E: ToSse + Clone + Send + Sync + 'static> {
    stream: ReceiverStream<Result<axum::response::sse::Event, Infallible>>,
    _cleanup_guard: ConnectionGuard<E>,
}

impl<E: ToSse + Clone + Send + Sync + 'static> StreamWithGuard<E> {
    pub const fn new(
        stream: ReceiverStream<Result<axum::response::sse::Event, Infallible>>,
        cleanup_guard: ConnectionGuard<E>,
    ) -> Self {
        Self {
            stream,
            _cleanup_guard: cleanup_guard,
        }
    }
}

impl<E: ToSse + Clone + Send + Sync + 'static> futures_util::Stream for StreamWithGuard<E> {
    type Item = Result<axum::response::sse::Event, Infallible>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::pin::Pin::new(&mut self.stream).poll_next(cx)
    }
}

pub async fn create_sse_stream<E: ToSse + Clone + Send + Sync + 'static>(
    request_context: RequestContext,
    broadcaster: Arc<GenericBroadcaster<E>>,
    stream_name: &str,
) -> impl IntoResponse {
    let user_id = request_context.user_id().clone();
    let user_id_str = user_id.to_string();
    let conn_id = systemprompt_identifiers::ConnectionId::generate();
    let conn_id_str = conn_id.as_str().to_owned();

    tracing::info!(user_id = %user_id_str, conn_id = %conn_id_str, stream = %stream_name, "SSE stream opened");

    let (tx, rx) = mpsc::channel(1024);

    if !broadcaster.register(&user_id, &conn_id, tx.clone()).await {
        tracing::warn!(user_id = %user_id_str, stream = %stream_name, "SSE stream rejected: per-user connection cap reached");
        return ApiHttpError::from(ApiError::rate_limited(
            "Per-user stream connection limit reached",
        ))
        .into_response();
    }

    let cleanup_guard = ConnectionGuard::new(broadcaster, user_id, conn_id);
    let stream = ReceiverStream::new(rx);
    let stream_with_guard = StreamWithGuard::<E>::new(stream, cleanup_guard);

    tracing::info!(user_id = %user_id_str, conn_id = %conn_id_str, stream = %stream_name, "SSE stream ready");

    Sse::new(stream_with_guard)
        .keep_alive(standard_keep_alive())
        .into_response()
}
