//! Bridge authentication handlers for the gateway router.
//!
//! The JWT/session paths funnel through `systemprompt_oauth`'s
//! `issue_bridge_access`. The durable PAT path consumes the same exchange code,
//! then mints a first-class API key via the users `ApiKeyService` (see
//! `device_link`) — the two domains are composed in this member, rather than
//! wiring an `oauth → users` edge into either domain crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::Request;
use axum::http::{HeaderMap, header};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use systemprompt_extension::router::ApiHttpError;
use systemprompt_identifiers::{JwtToken, headers};
use systemprompt_manifest::Config;
use systemprompt_models::auth::BEARER_PREFIX;
use systemprompt_oauth::services::{
    BridgeAccessRequest, BridgeAuthResult, BridgeExchangeRequest, BridgeOAuthClient,
    exchange_bridge_session_code, issue_bridge_access, provision_bridge_oauth_client,
};
use systemprompt_security::http::{ClientIp, resolve_client_ip, resolve_request_base_url};

use super::{BridgeAuthState, credential_error, oauth_error, users_error};

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

pub async fn pat(
    state: BridgeAuthState,
    request: Request,
) -> Result<Json<AuthResponse>, ApiHttpError> {
    let pat_token = extract_bearer(request.headers())
        .ok_or_else(|| ApiHttpError::unauthorized("Missing Authorization: Bearer <pat>"))?;

    let record = state
        .api_keys
        .verify(&pat_token)
        .await
        .map_err(users_error)?
        .ok_or_else(|| ApiHttpError::unauthorized("Invalid PAT"))?;

    let caller_ip = resolve_client_ip(
        request.headers(),
        request
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>(),
        &state.config.trusted_proxies,
    );
    let result = issue_bridge_access(
        &state.keys,
        state.analytics.as_ref(),
        state.sessions.as_ref(),
        state.users.as_ref(),
        bridge_access_from_pat(
            BridgeAccessRequest::bridge(
                &state.config,
                request.headers(),
                caller_ip,
                &record.user_id,
            ),
            &record,
        ),
    )
    .await
    .map_err(oauth_error)?;

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
    state: BridgeAuthState,
    ClientIp(caller_ip): ClientIp,
    headers: HeaderMap,
    Json(body): Json<SessionExchangeBody>,
) -> Result<Json<AuthResponse>, ApiHttpError> {
    if body.code.trim().is_empty() {
        return Err(ApiHttpError::bad_request("missing exchange code"));
    }

    let result = exchange_bridge_session_code(
        &state.oauth,
        state.analytics.as_ref(),
        state.sessions.as_ref(),
        state.users.as_ref(),
        BridgeExchangeRequest {
            request_headers: &headers,
            caller_ip,
            code: body.code.trim(),
            keys: &state.keys,
            config: &state.config,
        },
    )
    .await
    .map_err(oauth_error)?
    .ok_or_else(|| {
        ApiHttpError::unauthorized("exchange code invalid, expired, or already consumed")
    })?;

    Ok(Json(result.into()))
}

pub async fn provision_oauth_client(
    state: BridgeAuthState,
    request: Request,
) -> Result<Json<BridgeOAuthClient>, ApiHttpError> {
    let bearer = extract_bearer(request.headers())
        .ok_or_else(|| ApiHttpError::unauthorized("Missing Authorization: Bearer <bridge-jwt>"))?;

    let (claims, _user) = state
        .authenticator
        .decode_for_gateway(&JwtToken::new(bearer))
        .await
        .map_err(credential_error)?;

    let token_endpoint = build_token_endpoint(&state.config, request.headers())?;

    let result = provision_bridge_oauth_client(&state.oauth, &claims.user_id, token_endpoint)
        .await
        .map_err(oauth_error)?;

    Ok(Json(result))
}

fn build_token_endpoint(cfg: &Config, headers: &HeaderMap) -> Result<String, ApiHttpError> {
    let configured = url::Url::parse(&cfg.api_external_url)
        .map_err(|e| ApiHttpError::internal("Configured external URL is invalid", e))?;
    let raw_host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let base = resolve_request_base_url(raw_host, &configured);
    Ok(format!("{}/api/v1/core/oauth/token", base.as_str()))
}

fn extract_bearer(hdrs: &HeaderMap) -> Option<String> {
    let auth = hdrs.get(headers::AUTHORIZATION)?.to_str().ok()?;
    auth.strip_prefix(BEARER_PREFIX)
        .map(|s| s.trim().to_owned())
}
