//! Per-context SSE stream endpoints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Extension, State};
use axum::response::IntoResponse;
use systemprompt_events::ToSse;
use systemprompt_identifiers::ConnectionId;
use systemprompt_models::events::ContextSummary;
use systemprompt_models::{ContextEvent, RequestContext, SystemEventBuilder};
use systemprompt_traits::ContextProvider;
use tokio::sync::mpsc;

use systemprompt_extension::router::subscription_refused;

use super::{STREAM_BUFFER, StreamState, guarded_sse};

pub async fn stream_context_state(
    Extension(request_context): Extension<RequestContext>,
    State(state): State<StreamState>,
) -> impl IntoResponse {
    let user_id = request_context.user_id().clone();
    let conn_id = ConnectionId::generate();

    tracing::info!(user_id = %user_id, conn_id = %conn_id, "SSE stream opened");

    let (tx, rx) = mpsc::channel(STREAM_BUFFER);

    let Some(guard) = state
        .feeds
        .open_context(&user_id, &conn_id, tx.clone())
        .await
    else {
        return subscription_refused(&request_context, "contexts");
    };

    match state.contexts.list_contexts_with_stats(&user_id).await {
        Ok(contexts_with_stats) => {
            let snapshot_data: Vec<ContextSummary> = contexts_with_stats
                .into_iter()
                .map(ContextSummary::from)
                .collect();

            let snapshot_event: ContextEvent =
                SystemEventBuilder::contexts_snapshot(snapshot_data).into();

            tracing::info!(conn_id = %conn_id, "SSE snapshot sent");

            if let Ok(sse_event) = snapshot_event.to_sse()
                && tx.try_send(Ok(sse_event)).is_err()
            {
                tracing::error!(conn_id = %conn_id, "Failed to send snapshot");
            }
        },
        Err(e) => {
            tracing::error!(conn_id = %conn_id, error = %e, "Failed to create snapshot");
        },
    }

    tracing::info!(user_id = %user_id, conn_id = %conn_id, "SSE stream ready");

    guarded_sse(rx, guard)
}
