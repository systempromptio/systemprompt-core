//! Live analytics SSE stream endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::response::IntoResponse;
use systemprompt_api::kernel::create_sse_stream;
use systemprompt_models::RequestContext;

use crate::http::AnalyticsHttpState;

pub(super) async fn analytics_stream(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<AnalyticsHttpState>,
) -> impl IntoResponse {
    create_sse_stream(req_ctx, Arc::clone(&state.broadcaster), "Analytics").await
}
