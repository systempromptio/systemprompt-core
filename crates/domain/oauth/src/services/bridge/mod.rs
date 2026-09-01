//! Bridge session exchange code generation and consumption.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod provisioning;

pub use provisioning::{BridgeOAuthClient, provision_bridge_oauth_client};

use crate::error::{OauthError, OauthResult as Result};
use chrono::{Duration as ChronoDuration, Utc};
use http::HeaderMap;
use rand::Rng;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::IpAddr;
use systemprompt_identifiers::{
    ClientId, PolicyVersion, SessionId, SessionSource, TraceId, UserId, headers,
};
use systemprompt_models::Config;
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience};
use systemprompt_traits::{AnalyticsProvider, CreateSessionInput, ExtractSignals};

use crate::repository::{CreateExchangeCodeParams, OAuthRepository};
use crate::services::generation::{
    JwtConfig, JwtSigningParams, generate_access_token_jti, generate_jwt,
};

const DEFAULT_ACCESS_TTL_SECONDS: u64 = 3600;
const EXCHANGE_CODE_BYTES: usize = 32;
// Why: sized for a person reading the code off a screen and typing it into a
// terminal, not for the machine round-trip of the loopback-redirect path.
const EXCHANGE_CODE_TTL_SECONDS: i64 = 600;

#[derive(Debug, Clone, Serialize)]
pub struct BridgeAuthResult {
    pub token: String,
    pub ttl: u64,
    pub headers: HashMap<String, String>,
}

#[derive(Debug)]
pub struct BridgeAccessRequest<'a> {
    pub request_headers: &'a HeaderMap,
    pub caller_ip: Option<IpAddr>,
    pub user_id: &'a UserId,
    pub client_id: ClientId,
    pub session_source: SessionSource,
    pub ttl_seconds: u64,
}

pub async fn issue_bridge_access(
    repo: &OAuthRepository,
    analytics: &dyn AnalyticsProvider,
    request_headers: &HeaderMap,
    caller_ip: Option<IpAddr>,
    user_id: &UserId,
) -> Result<BridgeAuthResult> {
    issue_bridge_access_with(
        repo,
        analytics,
        BridgeAccessRequest {
            request_headers,
            caller_ip,
            user_id,
            client_id: ClientId::bridge(),
            session_source: SessionSource::Bridge,
            ttl_seconds: DEFAULT_ACCESS_TTL_SECONDS,
        },
    )
    .await
}

// Why: a client-supplied session id keeps ONE user's session continuous
// across token refreshes — it must never let a client attach itself to a
// session it does not own. After a user switch the bridge can replay the
// previous account's session header; adopting it would mint a token whose
// session row belongs to someone else, which the gateway's attestation
// then rejects on every call ("session user mismatch"). Adopt only a
// session this user already owns; anything else gets a fresh id.
async fn adopt_or_mint_session(
    analytics: &dyn AnalyticsProvider,
    request_headers: &HeaderMap,
    user_id: &UserId,
) -> SessionId {
    let requested_session = request_headers
        .get(headers::SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(SessionId::new);
    match requested_session {
        Some(requested) => match analytics.find_session_by_id(&requested).await {
            Ok(Some(existing)) if existing.user_id.as_ref() == Some(user_id) => requested,
            Ok(None) => requested,
            Ok(Some(_)) => {
                tracing::warn!(
                    session_id = %requested,
                    user_id = %user_id,
                    "requested session belongs to a different user; minting a fresh session",
                );
                SessionId::generate()
            },
            Err(e) => {
                tracing::warn!(error = %e, "session ownership lookup failed; minting a fresh session");
                SessionId::generate()
            },
        },
        None => SessionId::generate(),
    }
}

pub async fn issue_bridge_access_with(
    repo: &OAuthRepository,
    analytics: &dyn AnalyticsProvider,
    request: BridgeAccessRequest<'_>,
) -> Result<BridgeAuthResult> {
    let BridgeAccessRequest {
        request_headers,
        caller_ip,
        user_id,
        client_id,
        session_source,
        ttl_seconds,
    } = request;

    let auth_user = repo.get_authenticated_user(user_id).await?;

    let global_config = Config::get()?;

    let session_id = adopt_or_mint_session(analytics, request_headers, user_id).await;
    let trace_id = TraceId::generate();
    let policy_version = PolicyVersion::unversioned();

    let ttl_hours = i64::try_from((ttl_seconds / 3600).max(1)).unwrap_or(1);
    let config = build_bridge_jwt_config(&auth_user, ttl_hours);
    let signing = JwtSigningParams {
        issuer: &global_config.jwt_issuer,
    };
    let token = generate_jwt(
        &auth_user,
        config,
        generate_access_token_jti(),
        &session_id,
        &signing,
    )?;

    // Why: The JWT embeds `session_id`, but the hardened gateway validator only
    // honours tokens whose session row exists and is unrevoked. Persist the row
    // here so the token and its session are born together. Analytics is
    // captured from the credential-exchange request so the session is traceable
    // to the device that minted it.
    let session_analytics = analytics.extract_analytics(
        request_headers,
        ExtractSignals {
            caller_ip,
            ..Default::default()
        },
    );
    let expires_at = Utc::now() + ChronoDuration::seconds(i64::try_from(ttl_seconds).unwrap_or(0));
    analytics
        .create_session(CreateSessionInput {
            session_id: &session_id,
            user_id: Some(user_id),
            analytics: &session_analytics,
            session_source,
            is_bot: false,
            is_ai_crawler: false,
            expires_at,
        })
        .await
        .map_err(|e| OauthError::Session(e.to_string()))?;

    let hdrs = build_bridge_headers(&BridgeHeaderParams {
        user_id,
        session_id: &session_id,
        trace_id: &trace_id,
        client_id: &client_id,
        policy_version: &policy_version,
        session_source,
    });

    Ok(BridgeAuthResult {
        token,
        ttl: ttl_seconds,
        headers: hdrs,
    })
}

fn build_bridge_jwt_config(auth_user: &AuthenticatedUser, ttl_hours: i64) -> JwtConfig {
    JwtConfig {
        permissions: auth_user.permissions().to_vec(),
        // Why: the bridge's loopback proxy injects this token when forwarding
        // MCP traffic to `/api/v1/mcp/<svc>`; `validate_service_access` and
        // each server's RBAC require the `mcp` audience, so the token carries
        // `mcp` beside `bridge` (kept for the auth/`/v1/messages` paths).
        // Per-user, short TTL, loopback-only — the same trust as the OAuth
        // flow it replaces.
        audience: vec![JwtAudience::Bridge, JwtAudience::Mcp],
        expires_in_hours: Some(ttl_hours),
        resource: None,
        plugin_id: None,
        client_id: Some(ClientId::bridge()),
    }
}

struct BridgeHeaderParams<'a> {
    user_id: &'a UserId,
    session_id: &'a SessionId,
    trace_id: &'a TraceId,
    client_id: &'a ClientId,
    policy_version: &'a PolicyVersion,
    session_source: SessionSource,
}

fn build_bridge_headers(params: &BridgeHeaderParams<'_>) -> HashMap<String, String> {
    let mut hdrs = HashMap::new();
    hdrs.insert(headers::USER_ID.to_owned(), params.user_id.to_string());
    hdrs.insert(
        headers::SESSION_ID.to_owned(),
        params.session_id.to_string(),
    );
    hdrs.insert(headers::TRACE_ID.to_owned(), params.trace_id.to_string());
    hdrs.insert(headers::CLIENT_ID.to_owned(), params.client_id.to_string());
    hdrs.insert(
        headers::POLICY_VERSION.to_owned(),
        params.policy_version.to_string(),
    );
    hdrs.insert(
        headers::CALL_SOURCE.to_owned(),
        params.session_source.as_str().to_owned(),
    );
    hdrs
}

#[derive(Debug, Clone, Serialize)]
pub struct BridgeExchangeCode {
    pub code: String,
    pub expires_at: chrono::DateTime<Utc>,
}

pub async fn issue_bridge_exchange_code(
    repo: &OAuthRepository,
    user_id: &UserId,
) -> Result<BridgeExchangeCode> {
    let code = generate_exchange_code();
    let code_hash = hash_exchange_code(&code);
    let expires_at = Utc::now() + ChronoDuration::seconds(EXCHANGE_CODE_TTL_SECONDS);

    repo.create_bridge_exchange_code(CreateExchangeCodeParams {
        code_hash: &code_hash,
        user_id,
        expires_at,
    })
    .await?;

    Ok(BridgeExchangeCode { code, expires_at })
}

pub async fn exchange_bridge_session_code(
    repo: &OAuthRepository,
    analytics: &dyn AnalyticsProvider,
    request_headers: &HeaderMap,
    caller_ip: Option<IpAddr>,
    code: &str,
) -> Result<Option<BridgeAuthResult>> {
    let code_hash = hash_exchange_code(code);
    let Some(user_id) = repo.consume_bridge_exchange_code(&code_hash).await? else {
        return Ok(None);
    };
    let result = issue_bridge_access(repo, analytics, request_headers, caller_ip, &user_id).await?;
    Ok(Some(result))
}

pub fn hash_exchange_code(code: &str) -> String {
    let digest = Sha256::digest(code.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn generate_exchange_code() -> String {
    let mut raw = [0u8; EXCHANGE_CODE_BYTES];
    rand::rng().fill_bytes(&mut raw);
    let mut out = String::with_capacity(EXCHANGE_CODE_BYTES * 2);
    for byte in raw {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
