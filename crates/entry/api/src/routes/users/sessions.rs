//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_traits::AppContext as _;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RevokeAllResponse {
    pub revoked: u64,
}

pub async fn revoke_all_mine(
    Extension(req_ctx): Extension<RequestContext>,
    State(ctx): State<AppContext>,
) -> impl IntoResponse {
    let user_id = &req_ctx.auth.actor.user_id;
    let Some(provider) = ctx.analytics_provider() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "analytics provider unavailable",
        )
            .into_response();
    };
    match provider.revoke_all_sessions_for_user(user_id).await {
        Ok(count) => (StatusCode::OK, Json(RevokeAllResponse { revoked: count })).into_response(),
        Err(e) => {
            tracing::warn!(error = %e, user_id = %user_id, "revoke_all_sessions failed");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response()
        },
    }
}
