//! Live analytics SSE stream endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::response::IntoResponse;

use super::AnalyticsState;
use crate::routes::stream::create_sse_stream;
use systemprompt_models::RequestContext;

pub(super) async fn analytics_stream(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<AnalyticsState>,
) -> impl IntoResponse {
    create_sse_stream(
        req_ctx,
        Arc::clone(&state.broadcasters.analytics),
        "Analytics",
    )
    .await
}
