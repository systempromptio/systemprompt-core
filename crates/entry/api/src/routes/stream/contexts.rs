use axum::extract::{Extension, State};
use axum::response::sse::Sse;
use axum::response::IntoResponse;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

use super::{StreamState, StreamWithGuard};
use systemprompt_events::{standard_keep_alive, Broadcaster, ConnectionGuard, ToSse, CONTEXT_BROADCASTER};
use systemprompt_models::events::ContextSummary;
use systemprompt_models::{ContextEvent, RequestContext, SystemEventBuilder};
use systemprompt_traits::ContextProvider;

pub async fn stream_context_state(
    Extension(request_context): Extension<RequestContext>,
    State(state): State<StreamState>,
) -> impl IntoResponse {
    let user_id = request_context.user_id().clone();
    let user_id_str = user_id.to_string();
    let conn_id = uuid::Uuid::new_v4().to_string();

    tracing::info!(user_id = %user_id_str, conn_id = %conn_id, "SSE stream opened");

    let (tx, rx) = mpsc::unbounded_channel();

    CONTEXT_BROADCASTER
        .register(&user_id, &conn_id, tx.clone())
        .await;

    match state
        .context_provider
        .list_contexts_with_stats(&user_id_str)
        .await
    {
        Ok(contexts_with_stats) => {
            let snapshot_data: Vec<ContextSummary> = contexts_with_stats
                .into_iter()
                .map(ContextSummary::from)
                .collect();

            let snapshot_event: ContextEvent =
                SystemEventBuilder::contexts_snapshot(snapshot_data).into();

            tracing::info!(conn_id = %conn_id, "SSE snapshot sent");

            if let Ok(sse_event) = snapshot_event.to_sse() {
                if tx.send(Ok(sse_event)).is_err() {
                    tracing::error!(conn_id = %conn_id, "Failed to send snapshot");
                }
            }
        },
        Err(e) => {
            tracing::error!(conn_id = %conn_id, error = %e, "Failed to create snapshot");
        },
    }

    let cleanup_guard =
        ConnectionGuard::new(&CONTEXT_BROADCASTER, user_id.clone(), conn_id.clone());
    let stream = UnboundedReceiverStream::new(rx);
    let stream_with_guard = StreamWithGuard::<ContextEvent>::new(stream, cleanup_guard);

    tracing::info!(user_id = %user_id_str, conn_id = %conn_id, "SSE stream ready");

    Sse::new(stream_with_guard)
        .keep_alive(standard_keep_alive())
        .into_response()
}
