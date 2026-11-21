//! Test signing keys and JWT minting helpers.
//!
//! [`test_signing_keys`] builds the [`SigningKeys`] value every test hands to
//! the context or constructor under test. The RSA key is the committed fixture
//! from [`crate::keys`] and the manifest seed is the all-zero seed the fixture
//! secrets carry, so a value built here agrees with the keys an `AppContext`
//! resolves from the bootstrapped profile.

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, Header};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AccessTokenId, JwtToken, SessionId, UserId};
use systemprompt_models::auth::{
    AuthenticatedUser, CustomClaims, JwtAudience, JwtClaims, Permission, RateLimitTier,
    RegisteredClaims, TokenType, UserJwtRequest, UserType,
};
use systemprompt_security::jwt::{AdminTokenParams, JwtService};
use systemprompt_security::keys::SigningKeys;

const TEST_MANIFEST_SEED: [u8; 32] = [0; 32];

pub fn test_signing_keys() -> SigningKeys {
    SigningKeys::new(
        crate::keys::test_key(crate::keys::AUTHORITY_KEY_INDEX),
        &TEST_MANIFEST_SEED,
    )
    .expect("build the fixture signing keys")
}

pub fn mint_admin_jwt(user_id: &UserId, email: &str, issuer: &str) -> JwtToken {
    let session = SessionId::generate();
    let params = AdminTokenParams {
        user_id,
        session_id: &session,
        email,
        issuer,
        duration: Duration::hours(1),
        client_id: None,
    };
    JwtService::generate_admin_token(&test_signing_keys(), &params).expect("mint admin jwt")
}

pub fn mint_bridge_jwt(user_id: &UserId, email: &str, issuer: &str) -> JwtToken {
    let session = SessionId::generate();
    let now = Utc::now();
    let expiry = now + Duration::hours(1);

    let claims = JwtClaims {
        registered: RegisteredClaims {
            sub: user_id.to_string(),
            iat: now.timestamp(),
            exp: expiry.timestamp(),
            nbf: Some(now.timestamp()),
            iss: issuer.to_owned(),
            aud: JwtAudience::standard(),
            jti: uuid::Uuid::new_v4().to_string(),
        },
        custom: CustomClaims {
            scope: vec![Permission::User],
            username: email.to_owned(),
            email: email.to_owned(),
            user_type: UserType::User,
            roles: vec!["user".to_owned()],
            attributes: BTreeMap::new(),
            client_id: None,
            token_type: TokenType::Bearer,
            auth_time: now.timestamp(),
            session_id: Some(session),
            rate_limit_tier: Some(RateLimitTier::User),
            plugin_id: None,
            act: None,
            purpose: None,
            interaction: None,
            browser_session: false,
            ui_session: false,
        },
    };

    let keys = test_signing_keys();
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(keys.active_kid().to_owned());
    let token = encode(&header, &claims, keys.encoding_key()).expect("encode bridge jwt");
    JwtToken::new(token)
}

pub fn test_token_authority(
    issuer: &str,
) -> std::sync::Arc<dyn systemprompt_models::auth::TokenAuthority> {
    std::sync::Arc::new(systemprompt_security::DeploymentTokenAuthority::new(
        test_signing_keys(),
        issuer,
    ))
}

// The `Cookie` header a browser signed in through the web login sends: the
// token `mint_browser_session` mints (a browser session with no client id) for
// `user`, whose sign-in was `signed_in_ago_secs` ago, over a live session row.
pub async fn browser_session_cookie(
    pool: &DbPool,
    user: &UserId,
    signed_in_ago_secs: i64,
) -> anyhow::Result<String> {
    session_cookie_with(pool, user, signed_in_ago_secs, |_| {}).await
}

// The same cookie with the mint request changed by `edit` first, for a token
// that is deliberately not the web login's.
pub async fn session_cookie_with(
    pool: &DbPool,
    user: &UserId,
    signed_in_ago_secs: i64,
    edit: impl FnOnce(&mut UserJwtRequest),
) -> anyhow::Result<String> {
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(user.as_str())
        .fetch_one(pool.write_pool().as_ref())
        .await?;
    let session_id = SessionId::generate();
    crate::credential::seed_user_session(pool, user, &session_id).await?;
    let permissions = vec![Permission::User];
    let mut request = UserJwtRequest {
        user: AuthenticatedUser::new_with_roles(
            user.clone(),
            email.clone(),
            email,
            permissions.clone(),
            vec!["user".to_owned()],
        ),
        session_id,
        jti: AccessTokenId::generate(),
        permissions,
        audience: JwtAudience::standard(),
        ttl: Duration::hours(1),
        client_id: None,
        purpose: None,
        plugin_id: None,
        interaction: None,
        browser_session: true,
        ui_session: false,
        auth_time: Some(Utc::now().timestamp() - signed_in_ago_secs),
    };
    edit(&mut request);
    let token = test_token_authority(crate::app_context::FIXTURE_JWT_ISSUER)
        .mint_user_jwt(&request)
        .map_err(|e| anyhow::anyhow!("mint browser session token: {e}"))?;
    Ok(format!("access_token={token}"))
}

pub fn test_oauth_signing() -> systemprompt_oauth::OAuthSigning {
    systemprompt_oauth::OAuthSigning {
        keys: test_signing_keys(),
        authority: test_token_authority(crate::app_context::FIXTURE_JWT_ISSUER),
    }
}

pub fn mint_mcp_hop(
    server: &systemprompt_identifiers::McpServerId,
    user: systemprompt_models::auth::AuthenticatedUser,
    mcp_session: Option<&str>,
    issuer: &str,
) -> String {
    let identity = systemprompt_models::auth::McpHopIdentity {
        user_id: user.id.clone(),
        user_type: systemprompt_models::auth::UserType::from_permissions(&user.permissions),
        user: Some(user),
        session_id: SessionId::generate(),
        client_id: None,
        act_chain: Vec::new(),
        purpose: None,
        interaction: None,
        mcp_session: mcp_session.map(str::to_owned),
    };
    test_token_authority(issuer)
        .mint_mcp_hop(server, &identity)
        .expect("mint mcp hop token")
}
