use axum::routing::post;
use axum::Router;
use std::sync::Arc;
use systemprompt_core_analytics::EngagementRepository;
use systemprompt_runtime::AppContext;

mod handlers;

pub use handlers::{BatchResponse, EngagementBatchInput, EngagementState};

pub fn router(ctx: &AppContext) -> Router {
    let state = EngagementState {
        repo: Arc::new(
            EngagementRepository::new(ctx.db_pool())
                .expect("Failed to create EngagementRepository"),
        ),
    };

    Router::new()
        .route("/", post(handlers::record_engagement))
        .route("/batch", post(handlers::record_engagement_batch))
        .with_state(state)
}
