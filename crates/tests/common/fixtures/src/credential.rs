//! End-to-end credential fixtures.
//!
//! [`seed_admin_credential`] / [`seed_bridge_credential`] insert a user row,
//! an active `user_sessions` row, and mint a JWT whose `sub` and `session_id`
//! claims match — the three things every gateway / auth path checks. Tests
//! that just need "a credential that decode_for_gateway will accept" can
//! reach happy-path 200 in one call instead of stitching primitives together
//! and accepting `200 || 401 || 500`.

use std::collections::BTreeMap;

use anyhow::Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, Header, encode};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{JwtToken, SessionId, UserId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};
use systemprompt_security::keys::authority::{active_kid, encoding_key};
use uuid::Uuid;

use crate::jwt::install_test_signing_key;

#[derive(Debug, Clone)]
pub struct AuthedFixture {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub email: String,
    pub jwt: JwtToken,
}

pub async fn seed_user_row(pool: &DbPool, user_id: &UserId, email: &str) -> Result<()> {
    let p = pool
        .pool_arc()
        .map_err(|e| anyhow::anyhow!("read pool: {e}"))?;
    let uid = user_id.as_str();
    sqlx::query!(
        "INSERT INTO users (id, name, email) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        uid,
        uid,
        email,
    )
    .execute(p.as_ref())
    .await
    .map_err(|e| anyhow::anyhow!("seed user: {e}"))?;
    Ok(())
}

pub async fn seed_user_session(
    pool: &DbPool,
    user_id: &UserId,
    session_id: &SessionId,
) -> Result<()> {
    let p = pool
        .pool_arc()
        .map_err(|e| anyhow::anyhow!("read pool: {e}"))?;
    sqlx::query!(
        "INSERT INTO user_sessions (session_id, user_id, session_source) \
         VALUES ($1, $2, 'bridge') \
         ON CONFLICT (session_id) DO UPDATE SET user_id = EXCLUDED.user_id, revoked_at = NULL",
        session_id.as_str(),
        user_id.as_str(),
    )
    .execute(p.as_ref())
    .await
    .map_err(|e| anyhow::anyhow!("seed session: {e}"))?;
    Ok(())
}

fn mint_jwt_internal(
    user_id: &UserId,
    session_id: &SessionId,
    email: &str,
    issuer: &str,
    user_type: UserType,
    scope: Vec<Permission>,
    roles: Vec<String>,
    rate_limit_tier: RateLimitTier,
) -> JwtToken {
    install_test_signing_key();
    let now = Utc::now();
    let expiry = now + Duration::hours(1);
    let claims = JwtClaims {
        sub: user_id.to_string(),
        iat: now.timestamp(),
        exp: expiry.timestamp(),
        nbf: Some(now.timestamp()),
        iss: issuer.to_owned(),
        aud: JwtAudience::standard(),
        jti: Uuid::new_v4().to_string(),
        scope,
        username: email.to_owned(),
        email: email.to_owned(),
        user_type,
        roles,
        attributes: BTreeMap::new(),
        client_id: None,
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: Some(session_id.clone()),
        rate_limit_tier: Some(rate_limit_tier),
        plugin_id: None,
        act: None,
    };
    let kid = active_kid().expect("active kid present");
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_owned());
    let key = encoding_key().expect("encoding key present");
    let token = encode(&header, &claims, key).expect("encode jwt");
    JwtToken::new(token)
}

pub async fn seed_admin_credential(pool: &DbPool, email_base: &str) -> Result<AuthedFixture> {
    let user_id = UserId::new(format!("admin-{}", Uuid::new_v4()));
    let session_id = SessionId::generate();
    let email = unique_email(email_base, &user_id);
    seed_user_row(pool, &user_id, &email).await?;
    seed_user_session(pool, &user_id, &session_id).await?;
    let jwt = mint_jwt_internal(
        &user_id,
        &session_id,
        &email,
        "test-admin",
        UserType::Admin,
        vec![Permission::Admin],
        vec!["admin".to_owned(), "user".to_owned()],
        RateLimitTier::Admin,
    );
    Ok(AuthedFixture {
        user_id,
        session_id,
        email,
        jwt,
    })
}

pub async fn seed_bridge_credential(pool: &DbPool, email_base: &str) -> Result<AuthedFixture> {
    let user_id = UserId::new(format!("bridge-{}", Uuid::new_v4()));
    let session_id = SessionId::generate();
    let email = unique_email(email_base, &user_id);
    seed_user_row(pool, &user_id, &email).await?;
    seed_user_session(pool, &user_id, &session_id).await?;
    let jwt = mint_jwt_internal(
        &user_id,
        &session_id,
        &email,
        "test-bridge",
        UserType::User,
        vec![Permission::User],
        vec!["user".to_owned()],
        RateLimitTier::User,
    );
    Ok(AuthedFixture {
        user_id,
        session_id,
        email,
        jwt,
    })
}

fn unique_email(base: &str, user_id: &UserId) -> String {
    let (local, domain) = base.split_once('@').unwrap_or((base, "example.invalid"));
    format!("{local}+{}@{domain}", user_id.as_str())
}
