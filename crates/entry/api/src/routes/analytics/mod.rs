//! Analytics HTTP surface.
//!
//! Builds the router for event ingestion (single and batch) and the live
//! analytics SSE stream, wiring the shared [`AnalyticsState`] repositories.

mod events;
mod stream;

use anyhow::Result;
use axum::Router;
use axum::routing::{get, post};
use std::sync::Arc;
use systemprompt_analytics::{AnalyticsEventsRepository, EngagementRepository};
use systemprompt_content::ContentRepository;
use systemprompt_runtime::AppContext;

pub use events::AnalyticsState;

pub fn router(ctx: &AppContext) -> Result<Router> {
    let state = AnalyticsState {
        events: Arc::new(AnalyticsEventsRepository::new(ctx.db_pool())?),
        content: Arc::new(ContentRepository::new(ctx.db_pool())?),
        engagement: Arc::new(EngagementRepository::new(ctx.db_pool())?),
        content_routing: ctx.content_routing(),
    };

    Ok(routes().with_state(state))
}

fn routes() -> Router<AnalyticsState> {
    Router::new()
        .route("/events", post(events::record_event))
        .route("/events/batch", post(events::record_events_batch))
        .route("/stream", get(stream::analytics_stream))
}

/// Test-only seam: mount the analytics routes with a caller-supplied
/// `ContentRouting`, so the slug-resolution branch can be driven with a stub
/// that maps a page URL to a seeded content slug.
#[cfg(feature = "test-api")]
pub mod test_api {
    use super::{
        AnalyticsEventsRepository, AnalyticsState, ContentRepository, EngagementRepository, Router,
        routes,
    };
    use anyhow::Result;
    use std::sync::Arc;
    use systemprompt_models::ContentRouting;
    use systemprompt_runtime::AppContext;

    pub fn router_with_routing(
        ctx: &AppContext,
        content_routing: Option<Arc<dyn ContentRouting>>,
    ) -> Result<Router> {
        let state = AnalyticsState {
            events: Arc::new(AnalyticsEventsRepository::new(ctx.db_pool())?),
            content: Arc::new(ContentRepository::new(ctx.db_pool())?),
            engagement: Arc::new(EngagementRepository::new(ctx.db_pool())?),
            content_routing,
        };
        Ok(routes().with_state(state))
    }
}
