//! `/v1/bridge/decisions` — recent governance verdicts for the JWT subject.
//!
//! The bridge proxy already receives `x-systemprompt-request-id` on every
//! forwarded inference response, and the gateway keys its governance audit on
//! that same value. This endpoint is the read side of that correlation: it lets
//! the desktop app show what was actually decided about each request it made,
//! rather than asserting that governance happened.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Json;
use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode};
use chrono::{TimeZone, Utc};
use serde::Deserialize;
use systemprompt_identifiers::JwtToken;
use systemprompt_models::api::cloud::{BridgeGovernanceDecision, BridgeGovernanceDecisions};
use systemprompt_security::authz::recent_decisions_for_user;

use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

const DEFAULT_LIMIT: i64 = 200;
const MAX_LIMIT: i64 = 1000;
const DEFAULT_WINDOW_HOURS: i64 = 24;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct DecisionsQuery {
    since: Option<i64>,
    limit: Option<i64>,
}

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: systemprompt_runtime::AppContext,
    headers: HeaderMap,
    Query(query): Query<DecisionsQuery>,
) -> Result<Json<BridgeGovernanceDecisions>, (StatusCode, String)> {
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

    let since = query
        .since
        .and_then(|s| Utc.timestamp_opt(s, 0).single())
        .unwrap_or_else(|| Utc::now() - chrono::Duration::hours(DEFAULT_WINDOW_HOURS));
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let pool = ctx
        .db_pool()
        .pool_arc()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let rows = recent_decisions_for_user(&pool, claims.user_id.as_ref(), since, limit)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(BridgeGovernanceDecisions {
        decisions: rows
            .into_iter()
            .map(|r| BridgeGovernanceDecision {
                call_id: r.call_id,
                decision: r.decision,
                policy: r.policy,
                reason: r.reason,
                created_at: r.created_at,
            })
            .collect(),
    }))
}
