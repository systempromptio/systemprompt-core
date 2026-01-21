use axum::extract::Extension;
use axum::response::IntoResponse;

use crate::routes::stream::create_sse_stream;
use systemprompt_events::ANALYTICS_BROADCASTER;
use systemprompt_models::RequestContext;

pub async fn analytics_stream(Extension(req_ctx): Extension<RequestContext>) -> impl IntoResponse {
    create_sse_stream(&req_ctx, &ANALYTICS_BROADCASTER, "Analytics").await
}
