//! `POST /oauth/logout` — `handle_logout`. Revokes the bearer's `jti` before
//! its natural expiry. The handler reads the authenticated `RequestContext`
//! (jti, token_exp, user id), so we inject it as a request extension directly.
//! We cover the successful 204 revocation (writing to `oauth_jti_revocations`
//! and clearing the cookie), a non-UUID user logging out the same way, plus
//! the missing-jti and out-of-range expiry error branches.


use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::routes::oauth::authenticated_router;
use systemprompt_identifiers::{
    AccessTokenId, Actor, AgentName, ContextId, SessionId, TraceId, UserId,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_oauth::OAuthState;
use systemprompt_test_fixtures::{ensure_config, ensure_test_bootstrap, test_db_pool};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::setup_ctx;

const FUTURE_EXP: i64 = 4_102_444_800;

async fn logout_app() -> anyhow::Result<Router> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
        systemprompt_test_fixtures::test_signing_keys(),
    );
    Ok(authenticated_router().with_state(state))
}

async fn seed_user(user: &UserId) -> anyhow::Result<()> {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let p = pool.pool();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@logout.invalid", user.as_str()))
        .execute(p.as_ref())
        .await?;
    Ok(())
}

fn ctx_with(user: UserId, jti: &str, token_exp: i64) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::new("test-trace"),
        ContextId::generate(),
        AgentName::system(),
        Actor::user(user),
    )
    .with_jti(AccessTokenId::new(jti))
    .with_token_exp(token_exp)
}

fn logout_request(ctx: RequestContext) -> Request<Body> {
    let mut req = Request::builder()
        .method(http::Method::POST)
        .uri("/logout")
        .body(Body::empty())
        .expect("build");
    req.extensions_mut().insert(ctx);
    req
}

#[tokio::test]
async fn logout_valid_bearer_revokes_and_returns_204() -> anyhow::Result<()> {
    let user = UserId::new(Uuid::new_v4().to_string());
    seed_user(&user).await?;
    let app = logout_app().await?;
    let jti = format!("jti-{}", Uuid::new_v4());
    let exp = FUTURE_EXP;
    let resp = app
        .oneshot(logout_request(ctx_with(user, &jti, exp)))
        .await?;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "{}", resp.status());
    assert!(
        resp.headers().get(header::SET_COOKIE).is_some(),
        "logout must clear the access_token cookie"
    );
    Ok(())
}

#[tokio::test]
async fn logout_missing_jti_returns_invalid_request() -> anyhow::Result<()> {
    let user = UserId::new(Uuid::new_v4().to_string());
    let app = logout_app().await?;
    let ctx = RequestContext::new(
        SessionId::generate(),
        TraceId::new("test-trace"),
        ContextId::generate(),
        AgentName::system(),
        Actor::user(user),
    )
    .with_token_exp(FUTURE_EXP);
    let resp = app.oneshot(logout_request(ctx)).await?;
    assert!(resp.status().is_client_error(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn logout_non_uuid_user_revokes_and_returns_204() -> anyhow::Result<()> {
    let user = UserId::new(format!("seeded-admin-{}", Uuid::new_v4().simple()));
    seed_user(&user).await?;
    let app = logout_app().await?;
    let jti = format!("jti-{}", Uuid::new_v4());
    let resp = app
        .oneshot(logout_request(ctx_with(user, &jti, FUTURE_EXP)))
        .await?;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "{}", resp.status());

    let (_pool, ctx) = setup_ctx().await?;
    let revoked = ctx
        .oauth_repositories()
        .oauth
        .is_jti_revoked(&AccessTokenId::new(jti))
        .await?;
    assert!(revoked, "a non-UUID user's logout must record the jti");
    Ok(())
}

#[tokio::test]
async fn logout_out_of_range_expiry_returns_invalid_request() -> anyhow::Result<()> {
    let user = UserId::new(Uuid::new_v4().to_string());
    let app = logout_app().await?;
    let ctx = ctx_with(user, "some-jti", i64::MAX);
    let resp = app.oneshot(logout_request(ctx)).await?;
    assert!(resp.status().is_client_error(), "{}", resp.status());
    Ok(())
}
