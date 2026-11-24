//! User session revocation endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::{Extension, State};
use serde::Serialize;
use systemprompt_extension::router::ApiHttpError;
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_traits::DynSignInSessions;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RevokeAllResponse {
    pub revoked: u64,
}

pub async fn revoke_all_mine(
    Extension(req_ctx): Extension<RequestContext>,
    State(sessions): State<DynSignInSessions>,
) -> Result<Json<RevokeAllResponse>, ApiHttpError> {
    let user_id = &req_ctx.auth.actor.user_id;
    match sessions.revoke_all_sessions_for_user(user_id).await {
        Ok(count) => Ok(Json(RevokeAllResponse { revoked: count })),
        Err(e) => Err(ApiError::internal("Failed to revoke sessions", e).into()),
    }
}
