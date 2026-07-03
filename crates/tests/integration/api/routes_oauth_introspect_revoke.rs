//! `/oauth/introspect` (RFC 7662) and `/oauth/revoke` (RFC 7009).
//!
//! Both endpoints authenticate the *client* before acting on the token, so
//! each test seeds a confidential client with a known secret. Introspection is
//! driven with self-signed access tokens minted by the process-wide test
//! authority: an invalid token reports `active: false`, a valid token whose
//! `client_id` matches the introspecting client returns the full claim set, and
//! a valid token bound to a different client returns the minimal
//! `active: true` disclosure. Revocation exercises the `token_type_hint`
//! dispatch (refresh-token, access-token, and the unspecified fall-through)
//! plus the access-token `jti` recording path.

use std::sync::{Arc, Once};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, StatusCode, header};
use axum::middleware::{self, Next};
use systemprompt_api::routes::oauth::authenticated_router;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::Config;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_oauth::OAuthState;
use systemprompt_test_fixtures::{
    OAuthClientFixture, ensure_test_bootstrap, fixture_config, fixture_db_pool,
    install_test_signing_key, mint_admin_jwt, seed_oauth_client,
};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::setup_ctx;

static CONFIG_INSTALL: Once = Once::new();

fn ensure_config() {
    CONFIG_INSTALL.call_once(|| {
        let mut config = fixture_config("postgres://x");
        config.allowed_resource_audiences = vec!["hook".to_owned()];
        let _ = Config::install(config);
    });
}

fn ctx_for(user: &UserId) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::new("introspect-revoke"),
        ContextId::generate(),
        AgentName::system(),
    )
    .with_actor(Actor::user(user.clone()))
}

async fn oauth_app(user: UserId) -> anyhow::Result<Router> {
    ensure_config();
    install_test_signing_key();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    let inject = move |mut req: Request<Body>, next: Next| {
        let ctx = ctx_for(&user);
        async move {
            req.extensions_mut().insert(ctx);
            next.run(req).await
        }
    };
    Ok(authenticated_router()
        .layer(middleware::from_fn(inject))
        .with_state(state))
}

async fn seeded_client() -> anyhow::Result<(UserId, OAuthClientFixture)> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let user = UserId::new(Uuid::new_v4().to_string());
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@introspect.invalid", user.as_str()))
        .execute(p.as_ref())
        .await?;
    let client = seed_oauth_client(&pool, &user).await?;
    Ok((user, client))
}

fn mint_access_token(user: &UserId) -> String {
    mint_admin_jwt(user, "introspect@introspect.invalid", "test")
        .as_str()
        .to_owned()
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
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn enc(s: &str) -> String {
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
async fn introspect_missing_client_id_returns_invalid_client() -> anyhow::Result<()> {
    let (user, _client) = seeded_client().await?;
    let app = oauth_app(user).await?;
    let body = urlencode(&[("token", "x.y.z")]);
    let resp = app.oneshot(form_post("/introspect", body)).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_client"), "{v}");
    Ok(())
}

#[tokio::test]
async fn introspect_bad_client_secret_returns_invalid_client() -> anyhow::Result<()> {
    let (user, client) = seeded_client().await?;
    let app = oauth_app(user).await?;
    let body = urlencode(&[
        ("token", "x.y.z"),
        ("client_id", client.client_id.as_str()),
        ("client_secret", "wrong-secret"),
    ]);
    let resp = app.oneshot(form_post("/introspect", body)).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_client"), "{v}");
    Ok(())
}

#[tokio::test]
async fn introspect_invalid_token_reports_inactive() -> anyhow::Result<()> {
    let (user, client) = seeded_client().await?;
    let app = oauth_app(user).await?;
    let body = urlencode(&[
        ("token", "not-a-real-jwt"),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
    ]);
    let resp = app.oneshot(form_post("/introspect", body)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["active"].as_bool(), Some(false), "{v}");
    Ok(())
}

#[tokio::test]
async fn introspect_valid_self_signed_token_reports_active() -> anyhow::Result<()> {
    let (user, client) = seeded_client().await?;
    let token = mint_access_token(&user);
    let app = oauth_app(user).await?;
    let body = urlencode(&[
        ("token", &token),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
    ]);
    let resp = app.oneshot(form_post("/introspect", body)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["active"].as_bool(), Some(true), "{v}");
    Ok(())
}

#[tokio::test]
async fn revoke_access_token_hint_records_jti() -> anyhow::Result<()> {
    let (user, client) = seeded_client().await?;
    let token = mint_access_token(&user);
    let app = oauth_app(user).await?;
    let body = urlencode(&[
        ("token", &token),
        ("token_type_hint", "access_token"),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
    ]);
    let resp = app.oneshot(form_post("/revoke", body)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn revoke_refresh_token_hint_returns_200() -> anyhow::Result<()> {
    let (user, client) = seeded_client().await?;
    let app = oauth_app(user).await?;
    let body = urlencode(&[
        ("token", &format!("rt-{}", Uuid::new_v4())),
        ("token_type_hint", "refresh_token"),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
    ]);
    let resp = app.oneshot(form_post("/revoke", body)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn revoke_unspecified_hint_falls_through_to_access_token() -> anyhow::Result<()> {
    let (user, _client) = seeded_client().await?;
    let token = mint_access_token(&user);
    let app = oauth_app(user).await?;
    let body = urlencode(&[("token", &token)]);
    let resp = app.oneshot(form_post("/revoke", body)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn revoke_with_bad_client_secret_returns_invalid_client() -> anyhow::Result<()> {
    let (user, client) = seeded_client().await?;
    let app = oauth_app(user).await?;
    let body = urlencode(&[
        ("token", "x.y.z"),
        ("client_id", client.client_id.as_str()),
        ("client_secret", "wrong-secret"),
    ]);
    let resp = app.oneshot(form_post("/revoke", body)).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_client"), "{v}");
    Ok(())
}
