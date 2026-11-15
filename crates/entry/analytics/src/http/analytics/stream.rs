//! Live analytics SSE stream endpoint.
//!
//! The analytics event broadcaster is not in the extension state: the API
//! layers it onto this group's router alone, because the `Analytics` row of
//! its closed core declaration table is handed it. A request that arrives
//! without it is answered `503 Service Unavailable`; the handler never builds
//! a broadcaster of its own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::Extension;
use axum::response::IntoResponse;
use systemprompt_api::kernel::{ApiHttpError, create_sse_stream};
use systemprompt_events::AnalyticsBroadcaster;
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;

pub(super) async fn analytics_stream(
    Extension(req_ctx): Extension<RequestContext>,
    broadcaster: Option<Extension<Arc<AnalyticsBroadcaster>>>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let Some(Extension(broadcaster)) = broadcaster else {
        tracing::error!("Analytics stream served without the kernel's analytics broadcaster");
        return Err(ApiError::service_unavailable("Analytics stream unavailable").into());
    };
    Ok(create_sse_stream(req_ctx, broadcaster, "Analytics").await)
}
