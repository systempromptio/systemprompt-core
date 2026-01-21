mod events;
mod stream;

use anyhow::Result;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use systemprompt_analytics::AnalyticsEventsRepository;
use systemprompt_content::ContentRepository;
use systemprompt_runtime::AppContext;

pub use events::AnalyticsState;

pub fn router(ctx: &AppContext) -> Result<Router> {
    let state = AnalyticsState {
        events_repo: Arc::new(AnalyticsEventsRepository::new(ctx.db_pool())?),
        content_repo: Arc::new(ContentRepository::new(ctx.db_pool())?),
    };

    Ok(Router::new()
        .route("/events", post(events::record_event))
        .route("/events/batch", post(events::record_events_batch))
        .route("/stream", get(stream::analytics_stream))
        .with_state(state))
}
