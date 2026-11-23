//! Bridge authentication handlers for the gateway router.
//!
//! The JWT/session paths funnel through `systemprompt_oauth`'s
//! `issue_bridge_access`. The durable PAT path consumes the same exchange code,
//! then mints a first-class API key via the users `ApiKeyService` (see
//! `device_link`) — the two domains are composed at the entry layer, rather
//! than wiring an `oauth → users` edge into either domain crate.
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
use systemprompt_manifest::Config;
use systemprompt_models::auth::BEARER_PREFIX;
use systemprompt_oauth::services::{
    BridgeAccessRequest, BridgeAuthResult, BridgeExchangeRequest, BridgeOAuthClient,
    exchange_bridge_session_code, issue_bridge_access, provision_bridge_oauth_client,
};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AppContext as _, UserProvider};
use systemprompt_users::ApiKeyService;

use super::bridge_error::BridgeError;
use crate::kernel::{ApiHttpError, ClientIp, JwtContextExtractor, resolve_request_base_url};
use crate::services::middleware::client_addr::client_ip_from_request;

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub ttl: u64,
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_token: Option<String>,
}

impl From<BridgeAuthResult> for AuthResponse {
    fn from(r: BridgeAuthResult) -> Self {
        Self {
            token: r.token,
            ttl: r.ttl,
            headers: r.headers,
            device_token: r.device_token,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Capabilities {
    pub modes: Vec<&'static str>,
}

pub async fn capabilities() -> Json<Capabilities> {
    Json(Capabilities {
        modes: vec!["pat", "session", "oauth-client"],
    })
}

#[derive(Debug, Deserialize)]
pub struct SessionExchangeBody {
    pub code: String,
}

pub async fn pat(ctx: AppContext, request: Request) -> Result<Json<AuthResponse>, ApiHttpError> {
    let pat_token = extract_bearer(request.headers())
        .ok_or_else(|| ApiHttpError::unauthorized("Missing Authorization: Bearer <pat>"))?;

    let service = ApiKeyService::new(
        Arc::clone(ctx.user_repository()),
        systemprompt_users::ApiKeyLifetime::from_max_days(ctx.config().api_key_max_lifetime_days),
    );
    let record = service
        .verify(&pat_token)
        .await?
        .ok_or_else(|| ApiHttpError::unauthorized("Invalid PAT"))?;

    let analytics = ctx.analytics_provider();
    let caller_ip = client_ip_from_request(&request, ctx.config());
    let users = require_user_provider(&ctx)?;
    let result = issue_bridge_access(
        ctx.signing_keys(),
        analytics.as_ref(),
        ctx.session_provider()
            .ok_or_else(|| {
                ApiHttpError::internal("Session provider unavailable", "provider is not registered")
            })?
            .as_ref(),
        users.as_ref(),
        bridge_access_from_pat(
            BridgeAccessRequest::bridge(
                ctx.config(),
                request.headers(),
                caller_ip,
                &record.user_id,
            ),
            &record,
        ),
    )
    .await?;

    Ok(Json(result.into()))
}

fn bridge_access_from_pat<'a>(
    request: BridgeAccessRequest<'a>,
    record: &systemprompt_users::UserApiKey,
) -> BridgeAccessRequest<'a> {
    let request = request.with_purpose(record.purpose.clone());
    if record.is_interactive_host() {
        request.interactive()
    } else {
        request
    }
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

    let analytics = ctx.analytics_provider();
    let users = require_user_provider(&ctx)?;
    let result = exchange_bridge_session_code(
        &ctx.oauth_repositories().oauth,
        analytics.as_ref(),
        ctx.session_provider()
            .ok_or_else(|| {
                ApiHttpError::internal("Session provider unavailable", "provider is not registered")
            })?
            .as_ref(),
        users.as_ref(),
        BridgeExchangeRequest {
            request_headers: &headers,
            caller_ip,
            code: body.code.trim(),
            keys: ctx.signing_keys(),
            config: ctx.config(),
        },
    )
    .await?
    .ok_or_else(|| {
        ApiHttpError::unauthorized("exchange code invalid, expired, or already consumed")
    })?;

    Ok(Json(result.into()))
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

    let token_endpoint = build_token_endpoint(ctx.config(), request.headers())?;

    let result = provision_bridge_oauth_client(
        &ctx.oauth_repositories().oauth,
        &claims.user_id,
        token_endpoint,
    )
    .await?;

    Ok(Json(result))
}

fn build_token_endpoint(cfg: &Config, headers: &HeaderMap) -> Result<String, ApiHttpError> {
    let configured = url::Url::parse(&cfg.api_external_url)
        .map_err(|e| BridgeError::internal("Configured external URL is invalid", e))?;
    let raw_host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let base = resolve_request_base_url(raw_host, &configured);
    Ok(format!("{}/api/v1/core/oauth/token", base.as_str()))
}

fn extract_bearer(hdrs: &HeaderMap) -> Option<String> {
    let auth = hdrs.get(headers::AUTHORIZATION)?.to_str().ok()?;
    auth.strip_prefix(BEARER_PREFIX)
        .map(|s| s.trim().to_owned())
}

pub(super) fn require_user_provider(
    ctx: &AppContext,
) -> Result<Arc<dyn UserProvider>, ApiHttpError> {
    ctx.user_provider().ok_or_else(|| {
        ApiHttpError::internal("User provider unavailable", "provider is not registered")
    })
}
