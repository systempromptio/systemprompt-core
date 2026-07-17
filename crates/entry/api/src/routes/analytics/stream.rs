//! Live analytics SSE stream endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Extension;
use axum::response::IntoResponse;

use crate::routes::stream::create_sse_stream;
use systemprompt_events::ANALYTICS_BROADCASTER;
use systemprompt_models::RequestContext;

pub(super) async fn analytics_stream(
    Extension(req_ctx): Extension<RequestContext>,
) -> impl IntoResponse {
    create_sse_stream(req_ctx, &ANALYTICS_BROADCASTER, "Analytics").await
}
