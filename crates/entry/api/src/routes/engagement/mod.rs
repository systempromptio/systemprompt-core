//! Engagement-event ingestion routes.
//!
//! Builds the router for single and batch engagement records, wiring the
//! engagement, session, and content repositories into the shared handler state.

use anyhow::Result;
use axum::Router;
use axum::routing::post;
use std::sync::Arc;
use systemprompt_analytics::{EngagementRepository, SessionRepository};
use systemprompt_content::ContentRepository;
use systemprompt_runtime::AppContext;

mod handlers;

pub use handlers::{BatchResponse, EngagementBatchInput, EngagementState};

pub fn router(ctx: &AppContext) -> Result<Router> {
    let state = EngagementState {
        repo: Arc::new(EngagementRepository::new(ctx.db_pool())?),
        session_repo: Arc::new(SessionRepository::new(ctx.db_pool())?),
        content_repo: Arc::new(ContentRepository::new(ctx.db_pool())?),
        content_routing: ctx.content_routing(),
    };

    Ok(routes().with_state(state))
}

fn routes() -> Router<EngagementState> {
    Router::new()
        .route("/", post(handlers::record_engagement))
        .route("/batch", post(handlers::record_engagement_batch))
}

/// Test-only seam: mount the engagement routes with a caller-supplied router.
///
/// The supplied `ContentRouting` lets the slug-resolution and
/// conversion-marking branches be driven with a stub that maps a page URL to a
/// seeded content slug.
#[cfg(feature = "test-api")]
pub mod test_api {
    use super::{
        ContentRepository, EngagementRepository, EngagementState, Router, SessionRepository, routes,
    };
    use anyhow::Result;
    use std::sync::Arc;
    use systemprompt_models::ContentRouting;
    use systemprompt_runtime::AppContext;

    pub fn router_with_routing(
        ctx: &AppContext,
        content_routing: Option<Arc<dyn ContentRouting>>,
    ) -> Result<Router> {
        let state = EngagementState {
            repo: Arc::new(EngagementRepository::new(ctx.db_pool())?),
            session_repo: Arc::new(SessionRepository::new(ctx.db_pool())?),
            content_repo: Arc::new(ContentRepository::new(ctx.db_pool())?),
            content_routing,
        };
        Ok(routes().with_state(state))
    }
}
