use axum::extract::Extension;
use axum::response::sse::Sse;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use once_cell::sync::Lazy;
use std::convert::Infallible;
use std::sync::Arc;
use systemprompt_core_agent::services::ContextProviderService;
use systemprompt_core_events::{
    standard_keep_alive, Broadcaster, ConnectionGuard, GenericBroadcaster, A2A_BROADCASTER,
    AGUI_BROADCASTER,
};
use systemprompt_models::events::ToSse;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

pub mod contexts;

#[derive(Clone, Debug)]
pub struct StreamState {
    pub context_provider: Arc<ContextProviderService>,
}

pub fn stream_router(ctx: &AppContext) -> Router {
    let state = StreamState {
        context_provider: Arc::new(ContextProviderService::new(ctx.db_pool().clone())),
    };

    Router::new()
        .route("/contexts", get(contexts::stream_context_state))
        .route("/agui", get(stream_agui_events))
        .route("/a2a", get(stream_a2a_events))
        .with_state(state)
}

pub async fn stream_a2a_events(
    Extension(request_context): Extension<RequestContext>,
) -> impl IntoResponse {
    create_sse_stream(&request_context, &A2A_BROADCASTER, "A2A").await
}

pub async fn stream_agui_events(
    Extension(request_context): Extension<RequestContext>,
) -> impl IntoResponse {
    create_sse_stream(&request_context, &AGUI_BROADCASTER, "AgUI").await
}

#[derive(Debug)]
pub struct StreamWithGuard<E: ToSse + Clone + Send + Sync + 'static> {
    stream: UnboundedReceiverStream<Result<axum::response::sse::Event, Infallible>>,
    _cleanup_guard: ConnectionGuard<E>,
}

impl<E: ToSse + Clone + Send + Sync + 'static> StreamWithGuard<E> {
    pub fn new(
        stream: UnboundedReceiverStream<Result<axum::response::sse::Event, Infallible>>,
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
    request_context: &RequestContext,
    broadcaster: &'static Lazy<GenericBroadcaster<E>>,
    stream_name: &str,
) -> impl IntoResponse {
    let user_id = request_context.user_id().clone();
    let user_id_str = user_id.to_string();
    let conn_id = uuid::Uuid::new_v4().to_string();

    tracing::info!(user_id = %user_id_str, conn_id = %conn_id, stream = %stream_name, "SSE stream opened");

    let (tx, rx) = mpsc::unbounded_channel();

    broadcaster.register(&user_id, &conn_id, tx.clone()).await;

    let cleanup_guard = ConnectionGuard::new(broadcaster, user_id, conn_id.clone());
    let stream = UnboundedReceiverStream::new(rx);
    let stream_with_guard = StreamWithGuard::<E>::new(stream, cleanup_guard);

    tracing::info!(user_id = %user_id_str, conn_id = %conn_id, stream = %stream_name, "SSE stream ready");

    Sse::new(stream_with_guard)
        .keep_alive(standard_keep_alive())
        .into_response()
}
