//! `POST /v1/bridge/heartbeat` — bridge liveness reporting.
//!
//! Bridge processes report on a fixed cadence so the gateway can answer
//! "which devices are online right now" without inferring liveness from
//! inference traffic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Json;
use axum::http::HeaderMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::SessionId;
use systemprompt_models::bridge::manifest::{bridge_version_is_supported, min_bridge_version};
use systemprompt_oauth::repository::UpsertBridgeSession;
use systemprompt_runtime::AppContext;

use super::bridge_error::{BridgeError, authenticate_bridge};
use crate::error::ApiHttpError;
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

#[derive(Debug, Serialize)]
pub struct BridgeHeartbeatResponse {
    pub min_bridge_version: String,
    pub compatible: bool,
}

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    Json(payload): Json<BridgeHeartbeatRequest>,
) -> Result<Json<BridgeHeartbeatResponse>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;

    // Why: client attestation joins ai_requests.session_id to the bridge
    // session it was reported under; a heartbeat that names another session
    // would let one bridge vouch for traffic it never carried.
    if claims.session_id != payload.session_id {
        tracing::warn!(
            claimed_session = %claims.session_id,
            reported_session = %payload.session_id,
            "bridge heartbeat session does not match the token session; rejecting",
        );
        return Err(BridgeError::SessionMismatch.into());
    }

    let repo = &ctx.oauth_repositories().bridge_sessions;

    let floor = min_bridge_version();
    let compatible = bridge_version_is_supported(&payload.bridge_version, &floor);
    if !compatible {
        tracing::warn!(
            bridge_version = %payload.bridge_version,
            min_bridge_version = %floor,
            hostname = %payload.hostname,
            "bridge below the supported floor checked in",
        );
    }

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
    .map_err(BridgeError::from)?;

    Ok(Json(BridgeHeartbeatResponse {
        min_bridge_version: floor.to_string(),
        compatible,
    }))
}
