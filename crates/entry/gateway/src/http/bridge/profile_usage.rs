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

use axum::Json;
use axum::http::HeaderMap;
use chrono::Utc;
use systemprompt_models::api::cloud::BridgeProfileUsage;

use super::BridgeRouteState;
use super::error::{BridgeError, authenticate_bridge};
use systemprompt_extension::router::ApiHttpError;

pub async fn handle(
    state: BridgeRouteState,
    headers: HeaderMap,
) -> Result<Json<BridgeProfileUsage>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&state.authenticator, &headers).await?;

    let usage = state
        .repos
        .profile_usage
        .get_profile_usage(&claims.user_id, Utc::now())
        .await
        .map_err(|e| BridgeError::internal("profile usage lookup failed", e))?;

    Ok(Json(usage))
}
