//! `/track/engagement`: single and batch engagement records, with conversion
//! marking on the caller's session.
//!
//! The group is declared and nested at `/track/engagement`, so its routes
//! are relative to that prefix (`/` and `/batch`) and cannot answer outside
//! it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod handlers;

use axum::Router;
use axum::routing::post;

use super::AnalyticsHttpState;

pub use handlers::{BatchResponse, EngagementBatchInput};

pub fn router(state: AnalyticsHttpState) -> Router {
    Router::new()
        .route("/", post(handlers::record_engagement))
        .route("/batch", post(handlers::record_engagement_batch))
        .with_state(state)
}
