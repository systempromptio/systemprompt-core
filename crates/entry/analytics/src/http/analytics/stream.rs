//! Live analytics SSE stream endpoint.
//!
//! The handler subscribes through the analytics broadcaster the kernel hands
//! this route set; a route set without that conferral is refused at boot, so
//! the handle is always present. A subscription refused at the per-user
//! connection cap answers `429 Too Many Requests`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Extension, State};
use axum::response::{IntoResponse, Response};
use systemprompt_extension::router::{ApiHttpError, sse_response};
use systemprompt_identifiers::ConnectionId;
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;

use crate::http::AnalyticsHttpState;

pub(crate) async fn analytics_stream(
    State(state): State<AnalyticsHttpState>,
    Extension(req_ctx): Extension<RequestContext>,
) -> Response {
    let connection_id = ConnectionId::generate();
    match state
        .broadcaster
        .0
        .subscribe(req_ctx.user_id(), &connection_id)
        .await
    {
        Some(subscription) => sse_response(&req_ctx, subscription, "Analytics"),
        None => {
            tracing::warn!(user_id = %req_ctx.user_id(), stream = "Analytics", "SSE stream rejected: per-user connection cap reached");
            ApiHttpError::from(ApiError::rate_limited(
                "Per-user stream connection limit reached",
            ))
            .into_response()
        },
    }
}
