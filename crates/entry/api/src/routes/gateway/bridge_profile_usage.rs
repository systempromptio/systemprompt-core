//! `/v1/bridge/profile/usage` — per-user token usage and conversation summary.
//!
//! Returns rolling 24h / 7d / 30d windows of cost + tokens for the JWT
//! subject, the top 5 models by token share, and a conversation summary
//! grouped by model and by agent. Powers the bridge dashboard's profile tab.
//!
//! The derivation itself lives in `ProfileUsageService` so this route and the
//! server-rendered admin profile page cannot report different numbers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use chrono::Utc;
use systemprompt_identifiers::{JwtToken, UserId};
use systemprompt_models::api::cloud::BridgeProfileUsage;

use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: systemprompt_runtime::AppContext,
    headers: HeaderMap,
) -> Result<Json<BridgeProfileUsage>, (StatusCode, String)> {
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

    let user_id = UserId::new(claims.user_id.to_string());

    let usage = ctx
        .analytics_service()
        .profile_usage()
        .get_profile_usage(&user_id, Utc::now())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(usage))
}
