use axum::http::{HeaderMap, HeaderValue, StatusCode};
use systemprompt_agent::models::a2a::jsonrpc::NumberOrString;
use systemprompt_agent::services::a2a_server::auth::validate_oauth_for_request;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AccessTokenId, UserId};
use systemprompt_models::auth::{DynRequestAuthenticator, JwtAudience, Permission};
use systemprompt_test_fixtures::{
    fixture_request_authenticator, seed_user_row_with_roles, session_cookie_with, test_db_pool,
};

struct Caller {
    pool: DbPool,
    user: UserId,
    jti: AccessTokenId,
    headers: HeaderMap,
}

impl Caller {
    fn authenticator(&self) -> DynRequestAuthenticator {
        fixture_request_authenticator(&self.pool)
    }
}

fn bearer(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    h
}

async fn caller(roles: &[&str], scopes: &[Permission], audience: JwtAudience) -> Caller {
    let pool = test_db_pool().await;
    let user = UserId::new(uuid::Uuid::new_v4().to_string());
    let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
    seed_user_row_with_roles(&pool, &user, &format!("{user}@a2a.invalid"), &roles)
        .await
        .expect("seed user");
    let jti = AccessTokenId::generate();
    let minted = jti.clone();
    let scopes = scopes.to_vec();
    let cookie = session_cookie_with(&pool, &user, 0, move |request| {
        request.jti = minted;
        request.permissions = scopes;
        request.audience = vec![audience];
        request.browser_session = false;
    })
    .await
    .expect("mint token");
    let token = cookie.trim_start_matches("access_token=").to_owned();
    Caller {
        pool,
        user,
        jti,
        headers: bearer(&token),
    }
}

async fn admin() -> Caller {
    caller(&["admin", "user"], &[Permission::Admin], JwtAudience::A2a).await
}

async fn user(scopes: &[Permission]) -> Caller {
    caller(&["user"], scopes, JwtAudience::A2a).await
}

async fn validate(
    c: &Caller,
    required: &[Permission],
) -> Result<Option<systemprompt_agent::services::a2a_server::auth::AuthenticatedCaller>, StatusCode>
{
    validate_oauth_for_request(
        &c.headers,
        &NumberOrString::Number(1),
        required,
        Some(&c.authenticator()),
    )
    .await
    .map_err(|error| error.0)
}

#[tokio::test]
async fn validate_oauth_no_bearer_returns_unauthorized() {
    let c = admin().await;
    let err = validate_oauth_for_request(
        &HeaderMap::new(),
        &NumberOrString::Number(1),
        &[Permission::User],
        Some(&c.authenticator()),
    )
    .await
    .expect_err("should be unauthorized");
    assert_eq!(err.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn validate_oauth_empty_token_returns_unauthorized() {
    let c = admin().await;
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer "));
    let err = validate_oauth_for_request(
        &headers,
        &NumberOrString::Number(1),
        &[],
        Some(&c.authenticator()),
    )
    .await
    .expect_err("unauthorized");
    assert_eq!(err.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn validate_oauth_no_authenticator_returns_unauthorized() {
    let c = admin().await;
    let err = validate_oauth_for_request(&c.headers, &NumberOrString::Number(1), &[], None)
        .await
        .expect_err("no authenticator");
    assert_eq!(err.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn validate_oauth_invalid_token_returns_unauthorized() {
    let c = admin().await;
    let err = validate_oauth_for_request(
        &bearer("bad"),
        &NumberOrString::Number(1),
        &[],
        Some(&c.authenticator()),
    )
    .await
    .expect_err("invalid");
    assert_eq!(err.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn validate_oauth_admin_bypasses_scope_check() {
    let c = admin().await;
    let value = validate(&c, &[Permission::User])
        .await
        .expect("ok")
        .expect("Some");
    assert!(value.is_admin);
    assert_eq!(value.sub, c.user);
}

#[tokio::test]
async fn validate_oauth_token_of_another_audience_is_refused() {
    let c = caller(&["admin", "user"], &[Permission::Admin], JwtAudience::Mcp).await;
    assert_eq!(
        validate(&c, &[]).await.expect_err("refused"),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn validate_oauth_logged_out_session_is_refused() {
    let c = admin().await;
    sqlx::query("UPDATE user_sessions SET revoked_at = now() WHERE user_id = $1")
        .bind(c.user.as_str())
        .execute(c.pool.write_pool().as_ref())
        .await
        .expect("end the session");
    assert_eq!(
        validate(&c, &[Permission::User])
            .await
            .expect_err("refused"),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn validate_oauth_revoked_token_is_refused() {
    let c = admin().await;
    systemprompt_test_fixtures::oauth_fixture_repository(&c.pool)
        .revoke_jti(
            &c.jti,
            &c.user,
            chrono::Utc::now() + chrono::Duration::hours(1),
        )
        .await
        .expect("revoke");
    assert_eq!(
        validate(&c, &[Permission::User])
            .await
            .expect_err("refused"),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn validate_oauth_demoted_admin_loses_the_bypass_and_the_admin_scope() {
    let c = admin().await;
    sqlx::query("UPDATE users SET roles = ARRAY['user'] WHERE id = $1")
        .bind(c.user.as_str())
        .execute(c.pool.write_pool().as_ref())
        .await
        .expect("demote");
    assert_eq!(
        validate(&c, &[Permission::Admin])
            .await
            .expect_err("refused"),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn validate_oauth_user_with_matching_permission_succeeds() {
    let c = user(&[Permission::User]).await;
    let value = validate(&c, &[Permission::User])
        .await
        .expect("ok")
        .expect("Some");
    assert!(!value.is_admin);
    assert_eq!(value.sub, c.user);
}

#[tokio::test]
async fn validate_oauth_user_lacking_permission_forbidden() {
    let c = user(&[Permission::User]).await;
    assert_eq!(
        validate(&c, &[Permission::Admin])
            .await
            .expect_err("forbidden"),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn validate_oauth_empty_required_scopes_refuses_a_non_admin() {
    let c = user(&[Permission::User]).await;
    assert_eq!(
        validate(&c, &[])
            .await
            .expect_err("an empty requirement list is unsatisfiable"),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn validate_oauth_user_token_refused_for_a2a_scope() {
    let c = user(&[Permission::User]).await;
    assert_eq!(
        validate(&c, &[Permission::A2a])
            .await
            .expect_err("user scope is not the a2a scope"),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn validate_oauth_messaging_token_reaches_user_agent() {
    let c = user(&[Permission::A2a, Permission::User]).await;
    validate(&c, &[Permission::User])
        .await
        .expect("ok")
        .expect("Some");
}

#[tokio::test]
async fn validate_oauth_hook_token_refused_for_user_agent() {
    let c = user(&[Permission::HookGovern, Permission::HookTrack]).await;
    assert_eq!(
        validate(&c, &[Permission::User])
            .await
            .expect_err("hook scopes are not a user session"),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn validate_oauth_service_token_reaches_anonymous_agent() {
    let c = user(&[Permission::Service]).await;
    validate(&c, &[Permission::Anonymous])
        .await
        .expect("an anonymous requirement admits any token");
}
