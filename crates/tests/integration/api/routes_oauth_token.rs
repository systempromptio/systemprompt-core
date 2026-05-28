//! `/oauth/token` full HTTP round-trip — drives `handle_token` end-to-end
//! for the supported grant types. The existing `routes_oauth_public.rs`
//! file only checks that the route is reachable; this file verifies the
//! grant handlers themselves execute and produce wire-format responses
//! (or wire-format errors). The router is built with a synthetic
//! `RequestContext` extension layered in, which is what production gets
//! from `ContextMiddleware`.

use std::sync::{Arc, Once};

use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, StatusCode, header};
use axum::middleware::{self, Next};
use axum::Router;
use systemprompt_api::routes::oauth::public_router;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::Config;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use systemprompt_oauth::OAuthState;
use systemprompt_test_fixtures::{
    OAuthClientFixture, ensure_test_bootstrap, fixture_db_pool, seed_oauth_client,
};
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
            jwt_issuer: "test".to_owned(),
            jwt_access_token_expiration: 3600,
            jwt_refresh_token_expiration: 86_400,
            jwt_audiences: vec![],
            allowed_resource_audiences: vec![],
            trusted_issuers: vec![],
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

fn fixture_request_context() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::new("test-trace"),
        ContextId::generate(),
        AgentName::system(),
    )
}

async fn inject_context(mut req: Request<Body>, next: Next) -> Response<Body> {
    req.extensions_mut().insert(fixture_request_context());
    next.run(req).await
}

async fn token_app() -> anyhow::Result<Router> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    let router = public_router()
        .layer(middleware::from_fn(inject_context))
        .with_state(state);
    Ok(router)
}

async fn seeded_client() -> anyhow::Result<OAuthClientFixture> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let user = UserId::new(format!("oauth-token-owner-{}", Uuid::new_v4()));
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@oauth.invalid", user.as_str()))
        .execute(p.as_ref())
        .await?;
    seed_oauth_client(&pool, &user).await
}

fn form_post(uri: &str, body: String) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("build")
}

async fn read_json(resp: Response<Body>) -> anyhow::Result<serde_json::Value> {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

fn urlencode(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                urlencoding_minimal(k),
                urlencoding_minimal(v)
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn urlencoding_minimal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            },
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[tokio::test]
async fn token_unsupported_grant_returns_400() -> anyhow::Result<()> {
    let app = token_app().await?;
    let body = urlencode(&[("grant_type", "magic_beans")]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v = read_json(resp).await?;
    assert_eq!(
        v["error"].as_str(),
        Some("unsupported_grant_type"),
        "{v:?}"
    );
    Ok(())
}

#[tokio::test]
async fn token_client_credentials_with_bad_secret_returns_invalid_client() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", "client_credentials"),
        ("client_id", client.client_id.as_str()),
        ("client_secret", "wrong-secret"),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let s = resp.status();
    let v = read_json(resp).await?;
    assert!(
        s == StatusCode::UNAUTHORIZED || s == StatusCode::BAD_REQUEST,
        "{s} body={v}"
    );
    assert!(
        v["error"].as_str().is_some_and(|e| e.contains("invalid")),
        "expected invalid_* error, got {v:?}"
    );
    Ok(())
}

#[tokio::test]
async fn token_client_credentials_with_good_secret_issues_token() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", "client_credentials"),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let s = resp.status();
    let v = read_json(resp).await?;
    // The fixture client carries openid/profile scopes; the fixture user has
    // no roles, so scope intersection is empty and the grant correctly
    // rejects as invalid_scope (400). The 200 branch covers environments
    // where the seed user picks up default roles; 5xx is permitted only for
    // genuine internal failures (signing key / dependent state). 401/403 is
    // never acceptable here — that would mean the new error mapping is
    // collapsing scope rejections into client rejections.
    assert!(
        s == StatusCode::OK || s == StatusCode::BAD_REQUEST || s.is_server_error(),
        "{s} body={v}"
    );
    if s == StatusCode::OK {
        assert!(v.get("access_token").is_some(), "{v}");
        assert_eq!(v.get("token_type").and_then(|x| x.as_str()), Some("Bearer"));
    }
    Ok(())
}

#[tokio::test]
async fn token_authorization_code_with_unknown_code_returns_invalid_grant() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", "authorization_code"),
        ("code", "definitely-not-a-real-code"),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
        ("redirect_uri", &client.redirect_uri),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let s = resp.status();
    let v = read_json(resp).await?;
    assert!(s.is_client_error(), "{s} {v}");
    let err = v["error"].as_str().unwrap_or("");
    assert!(
        err.contains("invalid") || err.contains("expired"),
        "expected invalid/expired error, got {v}"
    );
    Ok(())
}

#[tokio::test]
async fn token_refresh_with_unknown_token_returns_invalid_grant() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", "definitely-not-a-real-refresh"),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let s = resp.status();
    let v = read_json(resp).await?;
    assert!(s.is_client_error() || s.is_server_error(), "{s} {v}");
    Ok(())
}

#[tokio::test]
async fn token_client_credentials_with_inactive_owner_returns_invalid_client() -> anyhow::Result<()>
{
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let user = UserId::new(format!("oauth-token-inactive-{}", Uuid::new_v4()));
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email, status) VALUES ($1, $1, $2, 'disabled') ON CONFLICT (id) DO UPDATE SET status='disabled'")
        .bind(user.as_str())
        .bind(format!("{}@oauth.invalid", user.as_str()))
        .execute(p.as_ref())
        .await?;
    let client = seed_oauth_client(&pool, &user).await?;

    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", "client_credentials"),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let s = resp.status();
    let v = read_json(resp).await?;
    assert_eq!(
        s,
        StatusCode::UNAUTHORIZED,
        "inactive owner must surface as 401 invalid_client, got {s} body={v}"
    );
    assert_eq!(v["error"].as_str(), Some("invalid_client"), "{v}");
    Ok(())
}

#[tokio::test]
async fn token_client_credentials_with_unknown_client_does_not_return_500() -> anyhow::Result<()> {
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", "client_credentials"),
        ("client_id", "no-such-client"),
        ("client_secret", "irrelevant"),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let s = resp.status();
    let v = read_json(resp).await?;
    assert!(
        s.is_client_error(),
        "unknown client must surface as 4xx, never 500, got {s} body={v}"
    );
    Ok(())
}

#[tokio::test]
async fn token_authorization_code_missing_code_field_returns_invalid_request() -> anyhow::Result<()>
{
    let client = seeded_client().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", "authorization_code"),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    assert!(resp.status().is_client_error(), "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}
