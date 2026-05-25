//! `POST /v1/bridge/heartbeat` — bridge processes report liveness here on a
//! fixed cadence so the gateway can answer "which devices are online right
//! now" without inferring liveness from inference traffic.

use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use systemprompt_identifiers::{JwtToken, SessionId};
use systemprompt_oauth::repository::{BridgeSessionRepository, UpsertBridgeSession};
use systemprompt_runtime::AppContext;

use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

#[derive(Debug, Deserialize)]
pub struct BridgeHeartbeatRequest {
    pub session_id: SessionId,
    pub bridge_version: String,
    pub os: String,
    pub hostname: String,
    #[serde(default)]
    pub last_activity_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub forwarded_total: i64,
    #[serde(default)]
    pub tokens_in_total: i64,
    #[serde(default)]
    pub tokens_out_total: i64,
}

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    Json(payload): Json<BridgeHeartbeatRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let credential = extract_credential(&headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_owned(),
        )
    })?;
    let claims = jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let repo = BridgeSessionRepository::new(ctx.db_pool()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("bridge session repo unavailable: {e}"),
        )
    })?;

    repo.upsert(UpsertBridgeSession {
        session_id: payload.session_id,
        user_id: claims.user_id,
        bridge_version: payload.bridge_version,
        os: payload.os,
        hostname: payload.hostname,
        last_activity_at: payload.last_activity_at,
        forwarded_total: payload.forwarded_total,
        tokens_in_total: payload.tokens_in_total,
        tokens_out_total: payload.tokens_out_total,
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("bridge heartbeat upsert failed: {e}"),
        )
    })?;

    Ok(StatusCode::NO_CONTENT)
}
