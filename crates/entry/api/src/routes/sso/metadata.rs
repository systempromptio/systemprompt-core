//! `GET /{id}/metadata` — the provider's service-provider metadata document,
//! when its protocol publishes one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use super::SsoRouteState;

pub(super) async fn handle_metadata(
    Path(id): Path<String>,
    State(state): State<SsoRouteState>,
) -> Response {
    match state.driver.metadata(&id).await {
        None => StatusCode::NOT_FOUND.into_response(),
        Some(Ok(doc)) => ([(header::CONTENT_TYPE, doc.content_type)], doc.body).into_response(),
        Some(Err(err)) => {
            tracing::warn!(provider_id = %id, error = %err, "SSO metadata unavailable");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        },
    }
}
