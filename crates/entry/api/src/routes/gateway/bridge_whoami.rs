//! `GET /v1/bridge/whoami` — identity envelope for the bridge profile tab.
//!
//! Decodes the bearer JWT, looks up the user record for email / display name
//! / roles, and returns the subset the gateway can authoritatively answer.
//! Fields the gateway has no source for (`tenant_id`, `provider`) are not
//! emitted; the bridge falls back to its locally verified identity snapshot
//! for those.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use serde::Serialize;
use systemprompt_identifiers::{JwtToken, UserId};
use systemprompt_runtime::AppContext;
use systemprompt_users::UserRepository;

use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

#[derive(Debug, Serialize)]
pub struct WhoamiResponse {
    pub user_id: UserId,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub roles: Vec<String>,
}

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
) -> Result<Json<WhoamiResponse>, (StatusCode, String)> {
    let credential = extract_credential(&headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_owned(),
        )
    })?;
    let (claims, _user) = jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let repo = UserRepository::new(ctx.db_pool())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let user = repo
        .find_by_id(&claims.user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("User not found: {}", claims.user_id.as_str()),
            )
        })?;

    Ok(Json(WhoamiResponse {
        user_id: claims.user_id,
        email: user.email,
        display_name: user.display_name.or(user.full_name),
        roles: user.roles,
    }))
}
