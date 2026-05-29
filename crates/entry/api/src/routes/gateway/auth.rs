//! Bridge authentication handlers for the gateway router.
//!
//! Exposes the credential-exchange endpoints a bridge uses to obtain a
//! short-lived access token: [`pat`] (personal access token), [`session`]
//! (one-time exchange code), [`mtls`] (enrolled device certificate), and
//! [`provision_oauth_client`] (dynamic OAuth client registration), plus
//! [`capabilities`] advertising the supported modes. All token-minting paths
//! funnel through `systemprompt_oauth`'s `issue_bridge_access`.

use axum::Json;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_identifiers::{JwtToken, UserId, headers};
use systemprompt_models::Config;
use systemprompt_models::auth::BEARER_PREFIX;
use systemprompt_oauth::services::{
    BridgeAuthResult, BridgeOAuthClient, exchange_bridge_session_code, issue_bridge_access,
    provision_bridge_oauth_client,
};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AnalyticsProvider, AppContext as _};
use systemprompt_users::{ApiKeyService, DeviceCertService};

use crate::services::middleware::JwtContextExtractor;

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub ttl: u64,
    pub headers: HashMap<String, String>,
}

impl From<BridgeAuthResult> for AuthResponse {
    fn from(r: BridgeAuthResult) -> Self {
        Self {
            token: r.token,
            ttl: r.ttl,
            headers: r.headers,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Capabilities {
    pub modes: Vec<&'static str>,
}

pub async fn capabilities() -> Json<Capabilities> {
    Json(Capabilities {
        modes: vec!["pat", "session", "mtls", "oauth-client"],
    })
}

#[derive(Debug, Deserialize)]
pub struct MtlsRequestBody {
    pub device_cert_fingerprint: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionExchangeBody {
    pub code: String,
}

pub async fn pat(
    ctx: AppContext,
    request: Request,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let pat_token = extract_bearer(request.headers()).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization: Bearer <pat>".into(),
        )
    })?;

    let service = ApiKeyService::new(ctx.db_pool())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let record = service
        .verify(&pat_token)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Invalid PAT".into()))?;

    let analytics = require_analytics(&ctx)?;
    let result = issue_bridge_access(
        ctx.db_pool(),
        analytics.as_ref(),
        request.headers(),
        &record.user_id,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(result.into()))
}

pub async fn session(
    ctx: AppContext,
    headers: HeaderMap,
    Json(body): Json<SessionExchangeBody>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    if body.code.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "missing exchange code".into()));
    }

    let analytics = require_analytics(&ctx)?;
    let result = exchange_bridge_session_code(
        ctx.db_pool(),
        analytics.as_ref(),
        &headers,
        body.code.trim(),
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "exchange code invalid, expired, or already consumed".into(),
        )
    })?;

    Ok(Json(result.into()))
}

pub async fn provision_oauth_client(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    request: Request,
) -> Result<Json<BridgeOAuthClient>, (StatusCode, String)> {
    let bearer = extract_bearer(request.headers()).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization: Bearer <bridge-jwt>".into(),
        )
    })?;

    let (claims, _user) = jwt_extractor
        .decode_for_gateway(&JwtToken::new(bearer))
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let user_id = UserId::new(claims.user_id.to_string());

    let token_endpoint =
        build_token_endpoint().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let result = provision_bridge_oauth_client(ctx.db_pool(), &user_id, token_endpoint)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(result))
}

fn build_token_endpoint() -> Result<String, String> {
    let cfg = Config::get().map_err(|e| e.to_string())?;
    Ok(format!(
        "{}/api/v1/core/oauth/token",
        cfg.api_external_url.trim_end_matches('/')
    ))
}

pub async fn mtls(
    ctx: AppContext,
    headers: HeaderMap,
    Json(body): Json<MtlsRequestBody>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let fingerprint = body.device_cert_fingerprint.trim();
    if fingerprint.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "missing device_cert_fingerprint".into(),
        ));
    }

    let service = DeviceCertService::new(ctx.db_pool())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let record = service
        .verify(fingerprint)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "device certificate not enrolled or revoked".into(),
            )
        })?;

    let analytics = require_analytics(&ctx)?;
    let result = issue_bridge_access(ctx.db_pool(), analytics.as_ref(), &headers, &record.user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(result.into()))
}

fn extract_bearer(hdrs: &HeaderMap) -> Option<String> {
    let auth = hdrs.get(headers::AUTHORIZATION)?.to_str().ok()?;
    auth.strip_prefix(BEARER_PREFIX)
        .map(|s| s.trim().to_owned())
}

fn require_analytics(ctx: &AppContext) -> Result<Arc<dyn AnalyticsProvider>, (StatusCode, String)> {
    ctx.analytics_provider().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "analytics provider unavailable".into(),
        )
    })
}
