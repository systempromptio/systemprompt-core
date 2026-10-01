//! User session listing and revocation endpoints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::{Extension, State};
use serde::Serialize;
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_runtime::AppContext;
use systemprompt_traits::AppContext as _;

use crate::error::ApiHttpError;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RevokeAllResponse {
    pub revoked: u64,
}

pub async fn revoke_all_mine(
    Extension(req_ctx): Extension<RequestContext>,
    State(ctx): State<AppContext>,
) -> Result<Json<RevokeAllResponse>, ApiHttpError> {
    let user_id = &req_ctx.auth.actor.user_id;
    let provider = ctx
        .session_provider()
        .ok_or_else(|| ApiHttpError::internal_error("Session provider unavailable"))?;
    match provider.revoke_all_sessions_for_user(user_id).await {
        Ok(count) => Ok(Json(RevokeAllResponse { revoked: count })),
        Err(e) => Err(ApiError::internal("Failed to revoke sessions", e).into()),
    }
}
