use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    ClientId, PolicyVersion, SessionId, SessionSource, TenantId, TraceId, UserId, headers,
};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::{Config, SecretsBootstrap};

use crate::repository::{CreateExchangeCodeParams, OAuthRepository};
use crate::services::generation::{
    JwtConfig, JwtSigningParams, generate_access_token_jti, generate_jwt,
};

const DEFAULT_ACCESS_TTL_SECONDS: u64 = 3600;
const EXCHANGE_CODE_BYTES: usize = 32;
const EXCHANGE_CODE_TTL_SECONDS: i64 = 120;

#[derive(Debug, Clone, Serialize)]
pub struct CoworkAuthResult {
    pub token: String,
    pub ttl: u64,
    pub headers: HashMap<String, String>,
}

pub async fn issue_cowork_access(pool: &DbPool, user_id: &UserId) -> Result<CoworkAuthResult> {
    issue_cowork_access_with(
        pool,
        user_id,
        ClientId::cowork(),
        SessionSource::Cowork,
        DEFAULT_ACCESS_TTL_SECONDS,
    )
    .await
}

pub async fn issue_cowork_access_with(
    pool: &DbPool,
    user_id: &UserId,
    client_id: ClientId,
    session_source: SessionSource,
    ttl_seconds: u64,
) -> Result<CoworkAuthResult> {
    let repo = OAuthRepository::new(pool)?;
    let auth_user = repo.get_authenticated_user(user_id).await?;

    let jwt_secret = SecretsBootstrap::jwt_secret()?;
    let global_config = Config::get()?;

    let session_id = SessionId::generate();
    let trace_id = TraceId::generate();
    let tenant_id = TenantId::new(global_config.jwt_issuer.clone());
    let policy_version = PolicyVersion::unversioned();

    let ttl_hours = i64::try_from((ttl_seconds / 3600).max(1)).unwrap_or(1);
    let config = JwtConfig {
        permissions: auth_user.permissions().to_vec(),
        audience: vec![JwtAudience::Cowork],
        expires_in_hours: Some(ttl_hours),
        resource: None,
    };
    let signing = JwtSigningParams {
        secret: jwt_secret,
        issuer: &global_config.jwt_issuer,
    };
    let token = generate_jwt(
        &auth_user,
        config,
        generate_access_token_jti(),
        &session_id,
        &signing,
    )?;

    let mut hdrs = HashMap::new();
    hdrs.insert(headers::USER_ID.to_string(), user_id.to_string());
    hdrs.insert(headers::SESSION_ID.to_string(), session_id.to_string());
    hdrs.insert(headers::TRACE_ID.to_string(), trace_id.to_string());
    hdrs.insert(headers::CLIENT_ID.to_string(), client_id.to_string());
    hdrs.insert(headers::TENANT_ID.to_string(), tenant_id.to_string());
    hdrs.insert(
        headers::POLICY_VERSION.to_string(),
        policy_version.to_string(),
    );
    hdrs.insert(
        headers::CALL_SOURCE.to_string(),
        session_source.as_str().to_string(),
    );

    Ok(CoworkAuthResult {
        token,
        ttl: ttl_seconds,
        headers: hdrs,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct CoworkExchangeCode {
    pub code: String,
    pub expires_at: chrono::DateTime<Utc>,
}

pub async fn issue_cowork_exchange_code(
    pool: &DbPool,
    user_id: &UserId,
) -> Result<CoworkExchangeCode> {
    let code = generate_exchange_code();
    let code_hash = hash_exchange_code(&code);
    let expires_at = Utc::now() + ChronoDuration::seconds(EXCHANGE_CODE_TTL_SECONDS);

    let repo = OAuthRepository::new(pool)?;
    repo.create_cowork_exchange_code(CreateExchangeCodeParams {
        code_hash: &code_hash,
        user_id,
        expires_at,
    })
    .await?;

    Ok(CoworkExchangeCode { code, expires_at })
}

pub async fn exchange_cowork_session_code(
    pool: &DbPool,
    code: &str,
) -> Result<Option<CoworkAuthResult>> {
    let code_hash = hash_exchange_code(code);
    let repo = OAuthRepository::new(pool)?;
    let Some(user_id) = repo.consume_cowork_exchange_code(&code_hash).await? else {
        return Ok(None);
    };
    let result = issue_cowork_access(pool, &user_id).await?;
    Ok(Some(result))
}

pub fn hash_exchange_code(code: &str) -> String {
    let digest = Sha256::digest(code.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn generate_exchange_code() -> String {
    let mut raw = [0u8; EXCHANGE_CODE_BYTES];
    rand::rng().fill_bytes(&mut raw);
    let mut out = String::with_capacity(EXCHANGE_CODE_BYTES * 2);
    for byte in raw {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}
