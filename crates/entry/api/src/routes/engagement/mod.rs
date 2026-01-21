use anyhow::Result;
use axum::routing::post;
use axum::Router;
use std::sync::Arc;
use systemprompt_analytics::EngagementRepository;
use systemprompt_content::ContentRepository;
use systemprompt_runtime::AppContext;

mod handlers;

pub use handlers::{BatchResponse, EngagementBatchInput, EngagementState};

pub fn router(ctx: &AppContext) -> Result<Router> {
    let state = EngagementState {
        repo: Arc::new(EngagementRepository::new(ctx.db_pool())?),
        content_repo: Arc::new(ContentRepository::new(ctx.db_pool())?),
    };

    Ok(Router::new()
        .route("/", post(handlers::record_engagement))
        .route("/batch", post(handlers::record_engagement_batch))
        .with_state(state))
}
