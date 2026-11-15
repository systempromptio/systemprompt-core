//! `/api/v1/analytics`: event ingestion (single and batch) and the live
//! analytics stream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod events;
mod stream;

use axum::Router;
use axum::routing::{get, post};

use super::AnalyticsHttpState;

pub fn router(state: AnalyticsHttpState) -> Router {
    Router::new()
        .route("/events", post(events::record_event))
        .route("/events/batch", post(events::record_events_batch))
        .route("/stream", get(stream::analytics_stream))
        .with_state(state)
}
