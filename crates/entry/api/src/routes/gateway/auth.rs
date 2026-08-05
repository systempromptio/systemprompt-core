//! Bridge authentication handlers for the gateway router.
//!
//! The JWT/session paths funnel through `systemprompt_oauth`'s
//! `issue_bridge_access`. The durable PAT path consumes the same exchange code,
//! then mints a first-class API key via the users `ApiKeyService` — the two
//! domains are composed here, at the entry layer, rather than wiring an
//! `oauth → users` edge into either domain crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::Request;
use axum::http::{HeaderMap, header};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_identifiers::{JwtToken, headers};
use systemprompt_models::Config;
use systemprompt_models::auth::BEARER_PREFIX;
use systemprompt_oauth::services::{
    BridgeAuthResult, BridgeOAuthClient, exchange_bridge_session_code, hash_exchange_code,
    issue_bridge_access, provision_bridge_oauth_client,
};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AnalyticsProvider, AppContext as _};
use systemprompt_users::{ApiKeyService, DeviceCertService, IssueApiKeyParams};

use crate::error::ApiHttpError;
use crate::services::middleware::JwtContextExtractor;
use crate::services::middleware::client_addr::{ClientIp, client_ip_from_request};
use crate::services::request_base_url;

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

#[derive(Debug, Deserialize)]
pub struct SessionPatBody {
    pub code: String,
    #[serde(default)]
    pub device_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DevicePatResponse {
    pub pat: String,
}

pub async fn pat(ctx: AppContext, request: Request) -> Result<Json<AuthResponse>, ApiHttpError> {
    let pat_token = extract_bearer(request.headers())
        .ok_or_else(|| ApiHttpError::unauthorized("Missing Authorization: Bearer <pat>"))?;

    let service = ApiKeyService::new(Arc::clone(ctx.user_repository()));
    let record = service
        .verify(&pat_token)
        .await?
        .ok_or_else(|| ApiHttpError::unauthorized("Invalid PAT"))?;

    let analytics = require_analytics(&ctx)?;
    let caller_ip = client_ip_from_request(&request);
    let result = issue_bridge_access(
        &ctx.oauth_repositories().oauth,
        analytics.as_ref(),
        request.headers(),
        caller_ip,
        &record.user_id,
    )
    .await?;

    Ok(Json(result.into()))
}

pub async fn session(
    ctx: AppContext,
    ClientIp(caller_ip): ClientIp,
    headers: HeaderMap,
    Json(body): Json<SessionExchangeBody>,
) -> Result<Json<AuthResponse>, ApiHttpError> {
    if body.code.trim().is_empty() {
        return Err(ApiHttpError::bad_request("missing exchange code"));
    }

    let analytics = require_analytics(&ctx)?;
    let result = exchange_bridge_session_code(
        &ctx.oauth_repositories().oauth,
        analytics.as_ref(),
        &headers,
        caller_ip,
        body.code.trim(),
    )
    .await?
    .ok_or_else(|| {
        ApiHttpError::unauthorized("exchange code invalid, expired, or already consumed")
    })?;

    Ok(Json(result.into()))
}

/// The PAT is returned exactly once; the bridge stores it and refreshes JWTs
/// silently from then on, with no recurring browser consent.
pub async fn session_pat(
    ctx: AppContext,
    Json(body): Json<SessionPatBody>,
) -> Result<Json<DevicePatResponse>, ApiHttpError> {
    let code = body.code.trim();
    if code.is_empty() {
        return Err(ApiHttpError::bad_request("missing exchange code"));
    }

    let device_name = body
        .device_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("bridge device-link");

    let pat = mint_device_pat(&ctx, code, device_name).await?;
    Ok(Json(DevicePatResponse { pat }))
}

async fn mint_device_pat(
    ctx: &AppContext,
    code: &str,
    device_name: &str,
) -> Result<String, ApiHttpError> {
    let repo = &ctx.oauth_repositories().oauth;
    let user_id = repo
        .consume_bridge_exchange_code(&hash_exchange_code(code))
        .await?
        .ok_or_else(|| {
            ApiHttpError::unauthorized("exchange code invalid, expired, or already consumed")
        })?;

    let service = ApiKeyService::new(Arc::clone(ctx.user_repository()));
    let issued = service
        .issue(IssueApiKeyParams {
            user_id: &user_id,
            name: device_name,
            expires_at: None,
        })
        .await?;

    Ok(issued.secret)
}

pub async fn provision_oauth_client(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    request: Request,
) -> Result<Json<BridgeOAuthClient>, ApiHttpError> {
    let bearer = extract_bearer(request.headers())
        .ok_or_else(|| ApiHttpError::unauthorized("Missing Authorization: Bearer <bridge-jwt>"))?;

    let (claims, _user) = jwt_extractor
        .decode_for_gateway(&JwtToken::new(bearer))
        .await?;

    let token_endpoint = build_token_endpoint(request.headers())?;

    let result = provision_bridge_oauth_client(
        &ctx.oauth_repositories().oauth,
        &claims.user_id,
        token_endpoint,
    )
    .await?;

    Ok(Json(result))
}

#[expect(
    clippy::result_large_err,
    reason = "ApiError carries response context that is intentionally large; boxing here would \
              propagate to every caller for negligible gain"
)]
// Why: the endpoint must reflect the host the client dialled — formatting
// `api_external_url` hands a remote client a loopback address. `resolve` falls
// back to the configured URL for a host outside the allowlist, so a forged
// `Host` cannot redirect the mint.
fn build_token_endpoint(headers: &HeaderMap) -> Result<String, ApiHttpError> {
    let cfg = Config::get().map_err(|e| ApiHttpError::internal_error(e.to_string()))?;
    let configured = url::Url::parse(&cfg.api_external_url)
        .map_err(|e| ApiHttpError::internal_error(e.to_string()))?;
    let raw_host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let base = request_base_url::resolve(raw_host, &configured);
    Ok(format!("{}/api/v1/core/oauth/token", base.as_str()))
}

pub async fn mtls(
    ctx: AppContext,
    ClientIp(caller_ip): ClientIp,
    headers: HeaderMap,
    Json(body): Json<MtlsRequestBody>,
) -> Result<Json<AuthResponse>, ApiHttpError> {
    let fingerprint = body.device_cert_fingerprint.trim();
    if fingerprint.is_empty() {
        return Err(ApiHttpError::bad_request("missing device_cert_fingerprint"));
    }

    let service = DeviceCertService::new(Arc::clone(ctx.user_repository()));
    let record = service
        .verify(fingerprint)
        .await?
        .ok_or_else(|| ApiHttpError::unauthorized("device certificate not enrolled or revoked"))?;

    let analytics = require_analytics(&ctx)?;
    let result = issue_bridge_access(
        &ctx.oauth_repositories().oauth,
        analytics.as_ref(),
        &headers,
        caller_ip,
        &record.user_id,
    )
    .await?;

    Ok(Json(result.into()))
}

fn extract_bearer(hdrs: &HeaderMap) -> Option<String> {
    let auth = hdrs.get(headers::AUTHORIZATION)?.to_str().ok()?;
    auth.strip_prefix(BEARER_PREFIX)
        .map(|s| s.trim().to_owned())
}

#[expect(
    clippy::result_large_err,
    reason = "ApiError carries response context that is intentionally large; boxing here would \
              propagate to every caller for negligible gain"
)]
fn require_analytics(ctx: &AppContext) -> Result<Arc<dyn AnalyticsProvider>, ApiHttpError> {
    ctx.analytics_provider()
        .ok_or_else(|| ApiHttpError::internal_error("analytics provider unavailable"))
}
