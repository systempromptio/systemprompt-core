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
use axum::http::HeaderMap;
use chrono::Utc;
use systemprompt_models::api::cloud::BridgeProfileUsage;

use super::bridge_error::{BridgeError, authenticate_bridge};
use crate::error::ApiHttpError;
use crate::services::middleware::JwtContextExtractor;

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: systemprompt_runtime::AppContext,
    headers: HeaderMap,
) -> Result<Json<BridgeProfileUsage>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;

    let usage = ctx
        .analytics_service()
        .profile_usage()
        .get_profile_usage(&claims.user_id, Utc::now())
        .await
        .map_err(|e| BridgeError::internal("profile usage lookup failed", e))?;

    Ok(Json(usage))
}
