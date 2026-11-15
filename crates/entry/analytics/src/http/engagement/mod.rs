//! `/track/engagement`: single and batch engagement records, with conversion
//! marking on the caller's session.
//!
//! The group is declared at the web root, so its routes carry the full
//! `/track/engagement` path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod handlers;

use axum::Router;
use axum::routing::post;
use systemprompt_models::modules::ApiPaths;

use super::AnalyticsHttpState;

pub use handlers::{BatchResponse, EngagementBatchInput};

pub fn router(state: AnalyticsHttpState) -> Router {
    Router::new()
        .route(ApiPaths::TRACK_ENGAGEMENT, post(handlers::record_engagement))
        .route(
            &format!("{}/batch", ApiPaths::TRACK_ENGAGEMENT),
            post(handlers::record_engagement_batch),
        )
        .with_state(state)
}
