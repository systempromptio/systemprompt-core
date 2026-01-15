use axum::extract::Extension;
use axum::response::IntoResponse;

use crate::routes::stream::create_sse_stream;
use systemprompt_core_events::ANALYTICS_BROADCASTER;
use systemprompt_models::RequestContext;

/// SSE endpoint for real-time analytics events.
///
/// Streams analytics events such as:
/// - SessionStarted: New session detected
/// - SessionEnded: Session closed with duration/stats
/// - PageView: Page view recorded
/// - EngagementUpdate: Engagement metrics update
/// - RealTimeStats: Periodic aggregated statistics
/// - Heartbeat: Keep-alive heartbeat
pub async fn analytics_stream(Extension(req_ctx): Extension<RequestContext>) -> impl IntoResponse {
    create_sse_stream(&req_ctx, &ANALYTICS_BROADCASTER, "Analytics").await
}
