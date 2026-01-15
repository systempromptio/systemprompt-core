mod events;
mod stream;

use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use systemprompt_core_analytics::AnalyticsEventsRepository;
use systemprompt_core_content::ContentRepository;
use systemprompt_runtime::AppContext;

pub use events::AnalyticsState;

pub fn router(ctx: &AppContext) -> Router {
    let state = AnalyticsState {
        events_repo: Arc::new(
            AnalyticsEventsRepository::new(ctx.db_pool())
                .expect("Failed to create AnalyticsEventsRepository"),
        ),
        content_repo: Arc::new(
            ContentRepository::new(ctx.db_pool()).expect("Failed to create ContentRepository"),
        ),
    };

    Router::new()
        .route("/events", post(events::record_event))
        .route("/events/batch", post(events::record_events_batch))
        .route("/stream", get(stream::analytics_stream))
        .with_state(state)
}
