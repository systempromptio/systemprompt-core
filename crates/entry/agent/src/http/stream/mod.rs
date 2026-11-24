//! Server-sent event stream routes for live A2A, `AgUI`, and context-state
//! feeds.
//!
//! Each route opens a per-user SSE connection through the member's
//! [`StreamFeeds`]: the feed registers the connection (with a per-user cap)
//! and returns the [`ConnectionGuard`] a [`StreamWithGuard`] holds, so the
//! connection deregisters on drop; the response emits keep-alive frames. The
//! API mounts the router at `/api/v1/stream` behind the user gate with a user
//! request context under the `stream` rate scope.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;
use std::future::Future;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Extension, State};
use axum::response::sse::Sse;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use systemprompt_agent::services::ContextProviderService;
use systemprompt_events::{ConnectionGuard, EventSender, ToSse, standard_keep_alive};
use systemprompt_extension::router::subscription_refused;
use systemprompt_identifiers::{ConnectionId, UserId};
use systemprompt_models::RequestContext;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

pub mod contexts;
mod feeds;

pub use feeds::StreamFeeds;

const STREAM_BUFFER: usize = 1024;

/// Router-scoped state of the stream routes.
#[derive(Clone, Debug)]
pub struct StreamState {
    pub contexts: Arc<ContextProviderService>,
    pub feeds: Arc<StreamFeeds>,
}

pub fn router(state: StreamState) -> Router {
    Router::new()
        .route("/contexts", get(contexts::stream_context_state))
        .route("/agui", get(stream_agui_events))
        .route("/a2a", get(stream_a2a_events))
        .with_state(state)
}

pub async fn stream_a2a_events(
    Extension(request_context): Extension<RequestContext>,
    State(state): State<StreamState>,
) -> impl IntoResponse {
    create_sse_stream(request_context, "A2A", |user, conn, tx| async move {
        state.feeds.open_a2a(&user, &conn, tx).await
    })
    .await
}

pub async fn stream_agui_events(
    Extension(request_context): Extension<RequestContext>,
    State(state): State<StreamState>,
) -> impl IntoResponse {
    create_sse_stream(request_context, "AgUI", |user, conn, tx| async move {
        state.feeds.open_agui(&user, &conn, tx).await
    })
    .await
}

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

pub(crate) fn guarded_sse<E: ToSse + Clone + Send + Sync + 'static>(
    rx: mpsc::Receiver<Result<axum::response::sse::Event, Infallible>>,
    guard: ConnectionGuard<E>,
) -> Response {
    Sse::new(StreamWithGuard::<E>::new(ReceiverStream::new(rx), guard))
        .keep_alive(standard_keep_alive())
        .into_response()
}

pub async fn create_sse_stream<E, F, Fut>(
    request_context: RequestContext,
    stream_name: &'static str,
    open: F,
) -> Response
where
    E: ToSse + Clone + Send + Sync + 'static,
    F: FnOnce(UserId, ConnectionId, EventSender) -> Fut,
    Fut: Future<Output = Option<ConnectionGuard<E>>>,
{
    let user_id = request_context.user_id().clone();
    let conn_id = ConnectionId::generate();

    tracing::info!(user_id = %user_id, conn_id = %conn_id, stream = %stream_name, "SSE stream opened");

    let (tx, rx) = mpsc::channel(STREAM_BUFFER);
    let Some(guard) = open(user_id.clone(), conn_id.clone(), tx).await else {
        return subscription_refused(&request_context, stream_name);
    };

    tracing::info!(user_id = %user_id, conn_id = %conn_id, stream = %stream_name, "SSE stream ready");

    guarded_sse(rx, guard)
}
