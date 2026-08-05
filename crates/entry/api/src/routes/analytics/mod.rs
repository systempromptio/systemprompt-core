//! Analytics HTTP surface.
//!
//! Builds the router for event ingestion (single and batch) and the live
//! analytics SSE stream, wiring the shared [`AnalyticsState`] repositories.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod events;
mod stream;

use axum::Router;
use axum::routing::{get, post};
use std::sync::Arc;
use systemprompt_models::ContentRouting;
use systemprompt_runtime::AppContext;

pub use events::AnalyticsState;

pub fn router(ctx: &AppContext) -> Router {
    routes().with_state(state(ctx, ctx.content_routing()))
}

fn state(ctx: &AppContext, content_routing: Option<Arc<dyn ContentRouting>>) -> AnalyticsState {
    let analytics = ctx.analytics_repositories();
    AnalyticsState {
        events: Arc::new(analytics.events.clone()),
        content: Arc::new(ctx.content_repositories().content.clone()),
        engagement: Arc::new(analytics.engagement.clone()),
        content_routing,
    }
}

fn routes() -> Router<AnalyticsState> {
    Router::new()
        .route("/events", post(events::record_event))
        .route("/events/batch", post(events::record_events_batch))
        .route("/stream", get(stream::analytics_stream))
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
