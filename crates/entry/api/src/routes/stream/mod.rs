//! Server-sent event stream routes for live A2A, `AgUI`, and context-state feeds.
//!
//! Each route opens a per-user SSE connection backed by a broadcaster from
//! `systemprompt_events`. [`create_sse_stream`] registers the connection (with
//! a per-user cap), wraps the receiver in a [`StreamWithGuard`] so the
//! [`ConnectionGuard`] deregisters it on drop, and emits keep-alive frames.

use axum::Router;
use axum::extract::Extension;
use axum::response::IntoResponse;
use axum::response::sse::Sse;
use axum::routing::get;
use std::convert::Infallible;
use std::sync::{Arc, LazyLock};
use systemprompt_agent::services::ContextProviderService;
use systemprompt_events::{
    A2A_BROADCASTER, AGUI_BROADCASTER, Broadcaster, ConnectionGuard, GenericBroadcaster, ToSse,
    standard_keep_alive,
};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

pub mod contexts;

#[derive(Clone, Debug)]
pub struct StreamState {
    pub context_provider: Arc<ContextProviderService>,
}

pub fn stream_router(ctx: &AppContext) -> anyhow::Result<Router> {
    let context_provider = ContextProviderService::new(ctx.db_pool())?;
    let state = StreamState {
        context_provider: Arc::new(context_provider),
    };

    Ok(Router::new()
        .route("/contexts", get(contexts::stream_context_state))
        .route("/agui", get(stream_agui_events))
        .route("/a2a", get(stream_a2a_events))
        .with_state(state))
}

pub async fn stream_a2a_events(
    Extension(request_context): Extension<RequestContext>,
) -> impl IntoResponse {
    create_sse_stream(request_context, &A2A_BROADCASTER, "A2A").await
}

pub async fn stream_agui_events(
    Extension(request_context): Extension<RequestContext>,
) -> impl IntoResponse {
    create_sse_stream(request_context, &AGUI_BROADCASTER, "AgUI").await
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

pub async fn create_sse_stream<E: ToSse + Clone + Send + Sync + 'static>(
    request_context: RequestContext,
    broadcaster: &'static LazyLock<GenericBroadcaster<E>>,
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
        return http::StatusCode::TOO_MANY_REQUESTS.into_response();
    }

    let cleanup_guard = ConnectionGuard::new(broadcaster, user_id, conn_id);
    let stream = ReceiverStream::new(rx);
    let stream_with_guard = StreamWithGuard::<E>::new(stream, cleanup_guard);

    tracing::info!(user_id = %user_id_str, conn_id = %conn_id_str, stream = %stream_name, "SSE stream ready");

    Sse::new(stream_with_guard)
        .keep_alive(standard_keep_alive())
        .into_response()
}
