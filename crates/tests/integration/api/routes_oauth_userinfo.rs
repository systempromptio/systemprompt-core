//! `/oauth/userinfo` (OIDC UserInfo), the `/.well-known` discovery variants
//! (trailing-slash routes and `OPTIONS` preflight), and the anonymous-session
//! client-validation branches on `/session`.
//!
//! UserInfo reads the bearer from the `Authorization` header and validates it
//! against the process-wide test authority: a missing header is
//! `invalid_request`, a malformed token is `invalid_token`, and a self-signed
//! admin token resolves to the identity claim set. The anonymous endpoint
//! rejects an unparseable `client_id` and an unregistered third-party client,
//! and issues a session for the first-party default.

use std::sync::{Arc, Once};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, StatusCode, header};
use systemprompt_api::routes::oauth::{public_router, wellknown_routes};
use systemprompt_identifiers::UserId;
use systemprompt_models::Config;
use systemprompt_oauth::OAuthState;
use systemprompt_test_fixtures::{fixture_config, install_test_signing_key, mint_admin_jwt};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::setup_ctx;

static CONFIG_INSTALL: Once = Once::new();

fn ensure_config() {
    CONFIG_INSTALL.call_once(|| {
        let _ = Config::install(fixture_config("postgres://x"));
    });
}

async fn oauth_state() -> anyhow::Result<OAuthState> {
    ensure_config();
    install_test_signing_key();
    let (_pool, ctx) = setup_ctx().await?;
    Ok(OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    ))
}

async fn userinfo_app() -> anyhow::Result<Router> {
    Ok(public_router().with_state(oauth_state().await?))
}

async fn read_json(resp: Response<Body>) -> anyhow::Result<serde_json::Value> {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

fn get_userinfo(auth: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(http::Method::GET)
        .uri("/userinfo");
    if let Some(value) = auth {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    builder.body(Body::empty()).expect("build")
}

fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("build")
}

#[tokio::test]
async fn userinfo_missing_authorization_returns_invalid_request() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    ensure_config();
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    let app = systemprompt_api::routes::oauth::authenticated_router().with_state(state);
    let resp = app.oneshot(get_userinfo(None)).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn userinfo_malformed_token_returns_invalid_token() -> anyhow::Result<()> {
    let state = oauth_state().await?;
    let app = systemprompt_api::routes::oauth::authenticated_router().with_state(state);
    let resp = app
        .oneshot(get_userinfo(Some("Bearer not-a-real-token")))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_token"), "{v}");
    Ok(())
}

#[tokio::test]
async fn userinfo_valid_token_returns_identity() -> anyhow::Result<()> {
    let state = oauth_state().await?;
    let app = systemprompt_api::routes::oauth::authenticated_router().with_state(state);
    let user = UserId::new(Uuid::new_v4().to_string());
    let token = mint_admin_jwt(&user, "userinfo@userinfo.invalid", "test");
    let resp = app
        .oneshot(get_userinfo(Some(&format!("Bearer {}", token.as_str()))))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["sub"].as_str(), Some(user.as_str()), "{v}");
    assert_eq!(
        v["email"].as_str(),
        Some("userinfo@userinfo.invalid"),
        "{v}"
    );
    Ok(())
}

#[tokio::test]
async fn wellknown_trailing_slash_variant_is_served() -> anyhow::Result<()> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_routes(&ctx);
    let resp = app
        .oneshot(
            Request::builder()
                .method(http::Method::GET)
                .uri("/.well-known/oauth-authorization-server/")
                .body(Body::empty())
                .expect("build"),
        )
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn wellknown_options_preflight_returns_200() -> anyhow::Result<()> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_routes(&ctx);
    let resp = app
        .oneshot(
            Request::builder()
                .method(http::Method::OPTIONS)
                .uri("/.well-known/openid-configuration")
                .body(Body::empty())
                .expect("build"),
        )
        .await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn anonymous_unparseable_client_id_returns_invalid_client() -> anyhow::Result<()> {
    let app = userinfo_app().await?;
    let body = serde_json::json!({ "client_id": "nonsense-without-a-known-prefix" });
    let resp = app.oneshot(json_post("/session", body)).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_client"), "{v}");
    Ok(())
}

#[tokio::test]
async fn anonymous_unregistered_third_party_returns_invalid_client() -> anyhow::Result<()> {
    let app = userinfo_app().await?;
    let body = serde_json::json!({
        "client_id": format!("client_{}", Uuid::new_v4().simple()),
    });
    let resp = app.oneshot(json_post("/session", body)).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{}", resp.status());
    let v = read_json(resp).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_client"), "{v}");
    Ok(())
}

#[tokio::test]
async fn anonymous_first_party_default_issues_session() -> anyhow::Result<()> {
    let app = userinfo_app().await?;
    let resp = app
        .oneshot(json_post("/session", serde_json::json!({})))
        .await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_success(), "expected 200, got {status} {v}");
    assert!(
        v["access_token"].as_str().is_some_and(|t| !t.is_empty()),
        "{v}"
    );
    assert_eq!(v["client_id"].as_str(), Some("sp_web"), "{v}");
    Ok(())
}
