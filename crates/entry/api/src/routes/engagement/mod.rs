//! Engagement-event ingestion routes.
//!
//! Builds the router for single and batch engagement records, wiring the
//! engagement, session, and content repositories into the shared handler state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::routing::post;
use std::sync::Arc;
use systemprompt_models::ContentRouting;
use systemprompt_runtime::AppContext;

mod handlers;

pub use handlers::{BatchResponse, EngagementBatchInput, EngagementState};

pub fn router(ctx: &AppContext) -> Router {
    routes().with_state(state(ctx, ctx.content_routing()))
}

fn state(ctx: &AppContext, content_routing: Option<Arc<dyn ContentRouting>>) -> EngagementState {
    let analytics = ctx.analytics_repositories();
    EngagementState {
        repo: Arc::new(analytics.engagement.clone()),
        session_repo: Arc::new(analytics.sessions.clone()),
        content_repo: Arc::new(ctx.content_repositories().content.clone()),
        content_routing,
    }
}

fn routes() -> Router<EngagementState> {
    Router::new()
        .route("/", post(handlers::record_engagement))
        .route("/batch", post(handlers::record_engagement_batch))
}

#[cfg(feature = "test-api")]
pub mod test_api {
    use super::{Router, routes, state};
    use std::sync::Arc;
    use systemprompt_models::ContentRouting;
    use systemprompt_runtime::AppContext;

    pub fn router_with_routing(
        ctx: &AppContext,
        content_routing: Option<Arc<dyn ContentRouting>>,
    ) -> Router {
        routes().with_state(state(ctx, content_routing))
    }
}
