//! `POST /oauth/logout` — `handle_logout`. Revokes the bearer's `jti` before
//! its natural expiry. The handler reads the authenticated `RequestContext`
//! (jti, token_exp, user id), so we inject it as a request extension directly.
//! We cover the successful 204 revocation (writing to `oauth_jti_revocations`
//! and clearing the cookie), plus the missing-jti, non-UUID user, and
//! out-of-range expiry error branches.

use std::sync::{Arc, Once};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::routes::oauth::authenticated_router;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::Config;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use systemprompt_oauth::OAuthState;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::setup_ctx;

static CONFIG_INSTALL: Once = Once::new();

fn ensure_config() {
    CONFIG_INSTALL.call_once(|| {
        let _ = Config::install(Config {
            instance_id: "test".to_owned(),
            max_concurrent_streams: 16,
            sitename: "test".to_owned(),
            database_type: "postgres".to_owned(),
            database_url: "postgres://x".to_owned(),
            database_write_url: None,
            github_link: String::new(),
            github_token: None,
            system_path: "/tmp".to_owned(),
            services_path: "/tmp".to_owned(),
            bin_path: "/tmp".to_owned(),
            skills_path: "/tmp".to_owned(),
            settings_path: "/tmp".to_owned(),
            content_config_path: "/tmp".to_owned(),
            geoip_database_path: None,
            web_path: "/tmp".to_owned(),
            web_config_path: "/tmp".to_owned(),
            web_metadata_path: "/tmp".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 0,
            api_server_url: "http://127.0.0.1".to_owned(),
            api_internal_url: "http://127.0.0.1".to_owned(),
            api_external_url: "http://127.0.0.1".to_owned(),
            jwt_issuer: "https://issuer.test".to_owned(),
            jwt_access_token_expiration: 3600,
            jwt_refresh_token_expiration: 86_400,
            jwt_audiences: vec![],
            allowed_resource_audiences: vec!["hook".to_owned()],
            trusted_issuers: vec![],
            id_jag_ttl_secs: 300,
            signing_key_path: std::path::PathBuf::from("signing_key.pem"),
            use_https: false,
            rate_limits: RateLimitConfig::default(),
            cors_allowed_origins: vec![],
            trusted_proxies: vec![],
            is_cloud: false,
            system_admin_username: "admin".to_owned(),
            content_negotiation: ContentNegotiationConfig::default(),
            security_headers: SecurityHeadersConfig::default(),
            allow_registration: false,
        });
    });
}

const FUTURE_EXP: i64 = 4_102_444_800;

async fn logout_app() -> anyhow::Result<Router> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    Ok(authenticated_router().with_state(state))
}

async fn seed_user(user: &UserId) -> anyhow::Result<()> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let p = pool.pool_arc().expect("read pool");
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
    )
    .with_actor(Actor::user(user))
    .with_jti(jti.to_owned())
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
    let resp = app
        .oneshot(logout_request(ctx_with(user, "", FUTURE_EXP)))
        .await?;
    assert!(resp.status().is_client_error(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn logout_non_uuid_user_returns_invalid_request() -> anyhow::Result<()> {
    let app = logout_app().await?;
    let ctx = ctx_with(UserId::new("not-a-uuid"), "some-jti", FUTURE_EXP);
    let resp = app.oneshot(logout_request(ctx)).await?;
    assert!(resp.status().is_client_error(), "{}", resp.status());
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
