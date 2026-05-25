//! Bridge session exchange code generation and consumption.

use crate::error::{OauthError, OauthResult as Result};
use chrono::{Duration as ChronoDuration, Utc};
use http::HeaderMap;
use rand::Rng;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    ClientId, PolicyVersion, SessionId, SessionSource, TraceId, UserId, headers,
};
use systemprompt_models::Config;
use systemprompt_models::auth::JwtAudience;
use systemprompt_traits::{AnalyticsProvider, CreateSessionInput};

use crate::repository::{CreateClientParams, CreateExchangeCodeParams, OAuthRepository};
use crate::services::generation::{
    JwtConfig, JwtSigningParams, generate_access_token_jti, generate_client_secret, generate_jwt,
    hash_client_secret,
};

const DEFAULT_ACCESS_TTL_SECONDS: u64 = 3600;
const EXCHANGE_CODE_BYTES: usize = 32;
const EXCHANGE_CODE_TTL_SECONDS: i64 = 120;

#[derive(Debug, Clone, Serialize)]
pub struct BridgeAuthResult {
    pub token: String,
    pub ttl: u64,
    pub headers: HashMap<String, String>,
}

#[derive(Debug)]
pub struct BridgeAccessRequest<'a> {
    pub request_headers: &'a HeaderMap,
    pub user_id: &'a UserId,
    pub client_id: ClientId,
    pub session_source: SessionSource,
    pub ttl_seconds: u64,
}

pub async fn issue_bridge_access(
    pool: &DbPool,
    analytics: &dyn AnalyticsProvider,
    request_headers: &HeaderMap,
    user_id: &UserId,
) -> Result<BridgeAuthResult> {
    issue_bridge_access_with(
        pool,
        analytics,
        BridgeAccessRequest {
            request_headers,
            user_id,
            client_id: ClientId::bridge(),
            session_source: SessionSource::Bridge,
            ttl_seconds: DEFAULT_ACCESS_TTL_SECONDS,
        },
    )
    .await
}

pub async fn issue_bridge_access_with(
    pool: &DbPool,
    analytics: &dyn AnalyticsProvider,
    request: BridgeAccessRequest<'_>,
) -> Result<BridgeAuthResult> {
    let BridgeAccessRequest {
        request_headers,
        user_id,
        client_id,
        session_source,
        ttl_seconds,
    } = request;

    let repo = OAuthRepository::new(pool)?;
    let auth_user = repo.get_authenticated_user(user_id).await?;

    let global_config = Config::get()?;

    // The bridge declares its stable session id via the `x-session-id` header on
    // the exchange request; the minted JWT must carry that id so it matches the
    // header the bridge sends on `/v1/messages`. Fall back to a fresh id for any
    // caller that does not supply one.
    let session_id = request_headers
        .get(headers::SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map_or_else(SessionId::generate, SessionId::new);
    let trace_id = TraceId::generate();
    let policy_version = PolicyVersion::unversioned();

    let ttl_hours = i64::try_from((ttl_seconds / 3600).max(1)).unwrap_or(1);
    let config = JwtConfig {
        permissions: auth_user.permissions().to_vec(),
        audience: vec![JwtAudience::Bridge],
        expires_in_hours: Some(ttl_hours),
        resource: None,
        plugin_id: None,
    };
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

    // The JWT embeds `session_id`, but the hardened gateway validator only
    // honours tokens whose session row exists and is unrevoked. Persist the row
    // here so the token and its session are born together. Analytics is
    // captured from the credential-exchange request so the session is traceable
    // to the device that minted it.
    let session_analytics = analytics.extract_analytics(request_headers, None);
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

    let mut hdrs = HashMap::new();
    hdrs.insert(headers::USER_ID.to_owned(), user_id.to_string());
    hdrs.insert(headers::SESSION_ID.to_owned(), session_id.to_string());
    hdrs.insert(headers::TRACE_ID.to_owned(), trace_id.to_string());
    hdrs.insert(headers::CLIENT_ID.to_owned(), client_id.to_string());
    hdrs.insert(
        headers::POLICY_VERSION.to_owned(),
        policy_version.to_string(),
    );
    hdrs.insert(
        headers::CALL_SOURCE.to_owned(),
        session_source.as_str().to_owned(),
    );

    Ok(BridgeAuthResult {
        token,
        ttl: ttl_seconds,
        headers: hdrs,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct BridgeExchangeCode {
    pub code: String,
    pub expires_at: chrono::DateTime<Utc>,
}

pub async fn issue_bridge_exchange_code(
    pool: &DbPool,
    user_id: &UserId,
) -> Result<BridgeExchangeCode> {
    let code = generate_exchange_code();
    let code_hash = hash_exchange_code(&code);
    let expires_at = Utc::now() + ChronoDuration::seconds(EXCHANGE_CODE_TTL_SECONDS);

    let repo = OAuthRepository::new(pool)?;
    repo.create_bridge_exchange_code(CreateExchangeCodeParams {
        code_hash: &code_hash,
        user_id,
        expires_at,
    })
    .await?;

    Ok(BridgeExchangeCode { code, expires_at })
}

pub async fn exchange_bridge_session_code(
    pool: &DbPool,
    analytics: &dyn AnalyticsProvider,
    request_headers: &HeaderMap,
    code: &str,
) -> Result<Option<BridgeAuthResult>> {
    let code_hash = hash_exchange_code(code);
    let repo = OAuthRepository::new(pool)?;
    let Some(user_id) = repo.consume_bridge_exchange_code(&code_hash).await? else {
        return Ok(None);
    };
    let result = issue_bridge_access(pool, analytics, request_headers, &user_id).await?;
    Ok(Some(result))
}

const BRIDGE_HOOK_CLIENT_SCOPES: &[&str] = &["hook:govern", "hook:track"];

fn bridge_hook_client_id(user_id: &UserId) -> ClientId {
    ClientId::new(format!("bridge:{}", user_id.as_str()))
}

/// Plaintext `client_secret` is returned only at creation/rotation time; the
/// database stores only the bcrypt hash. The bridge MUST persist this secret
/// on receipt — the server cannot re-emit it.
#[derive(Debug, Clone, Serialize)]
pub struct BridgeOAuthClient {
    pub client_id: ClientId,
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub token_endpoint: String,
}

pub async fn provision_bridge_oauth_client(
    pool: &DbPool,
    user_id: &UserId,
    token_endpoint: String,
) -> Result<BridgeOAuthClient> {
    let repo = OAuthRepository::new(pool)?;
    let client_id = bridge_hook_client_id(user_id);
    let secret = generate_client_secret();
    let secret_hash = hash_client_secret(&secret)?;

    let scopes: Vec<String> = BRIDGE_HOOK_CLIENT_SCOPES
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    let existing = repo.find_client_by_id(&client_id).await?;
    if existing.is_some() {
        repo.update_client_secret(&client_id, &secret_hash).await?;
    } else {
        let params = CreateClientParams {
            client_id: client_id.clone(),
            owner_user_id: user_id.clone(),
            client_secret_hash: secret_hash,
            client_name: format!("bridge hook client for {}", user_id.as_str()),
            redirect_uris: Vec::new(),
            grant_types: Some(vec!["client_credentials".to_owned()]),
            response_types: Some(Vec::new()),
            scopes: scopes.clone(),
            token_endpoint_auth_method: Some("client_secret_post".to_owned()),
            client_uri: None,
            logo_uri: None,
            contacts: None,
        };
        repo.create_client(params).await?;
    }

    Ok(BridgeOAuthClient {
        client_id,
        client_secret: secret,
        scopes,
        token_endpoint,
    })
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
