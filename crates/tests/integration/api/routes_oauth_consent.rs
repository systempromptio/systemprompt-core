//! `/consent` scope-consent endpoint — `handle_consent_get` and
//! `handle_consent_post`. The GET path renders the consent HTML page after
//! validating the requested scopes against the client's registered grant; the
//! POST path echoes the recorded decision. We drive the happy render, the
//! client-not-found, missing-scope, and invalid-scope error branches, plus
//! both allow/deny POST decisions.

use std::sync::{Arc, Once};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, StatusCode, header};
use systemprompt_api::routes::oauth::authenticated_router;
use systemprompt_identifiers::UserId;
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

use super::common::{empty_get, setup_ctx};

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

async fn consent_app() -> anyhow::Result<Router> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    Ok(authenticated_router().with_state(state))
}

async fn seeded_client() -> anyhow::Result<OAuthClientFixture> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let user = UserId::new(format!("consent-owner-{}", Uuid::new_v4()));
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@consent.invalid", user.as_str()))
        .execute(p.as_ref())
        .await?;
    seed_oauth_client(&pool, &user).await
}

async fn read_json(resp: Response<Body>) -> anyhow::Result<serde_json::Value> {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
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
async fn consent_get_renders_page_for_registered_scopes() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = consent_app().await?;
    let uri = format!(
        "/consent?client_id={}&scope=openid%20profile&state=abc",
        client.client_id.as_str()
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    let html = String::from_utf8_lossy(&bytes);
    assert!(
        html.contains("Grant Access"),
        "missing consent header: {html}"
    );
    assert!(
        html.contains("Access your openid data") && html.contains("Access your profile data"),
        "scope items not rendered: {html}"
    );
    Ok(())
}

#[tokio::test]
async fn consent_get_unknown_client_returns_invalid_client() -> anyhow::Result<()> {
    let app = consent_app().await?;
    let resp = app
        .oneshot(empty_get("/consent?client_id=no-such-client&scope=openid"))
        .await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_client"), "{v}");
    Ok(())
}

#[tokio::test]
async fn consent_get_missing_scope_returns_invalid_request() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = consent_app().await?;
    let uri = format!("/consent?client_id={}", client.client_id.as_str());
    let resp = app.oneshot(empty_get(&uri)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn consent_get_unregistered_scope_returns_invalid_scope() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = consent_app().await?;
    let uri = format!(
        "/consent?client_id={}&scope=admin",
        client.client_id.as_str()
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_scope"), "{v}");
    Ok(())
}

#[tokio::test]
async fn consent_post_allow_records_decision() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = consent_app().await?;
    let body = serde_json::json!({
        "client_id": client.client_id.as_str(),
        "scope": "openid profile",
        "state": "abc",
        "decision": "allow",
    });
    let resp = app.oneshot(json_post("/consent", body)).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let v = read_json(resp).await?;
    assert_eq!(v["status"].as_str(), Some("processed"), "{v}");
    assert_eq!(v["decision"].as_str(), Some("allow"), "{v}");
    Ok(())
}

#[tokio::test]
async fn consent_post_deny_records_decision() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = consent_app().await?;
    let body = serde_json::json!({
        "client_id": client.client_id.as_str(),
        "decision": "deny",
    });
    let resp = app.oneshot(json_post("/consent", body)).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let v = read_json(resp).await?;
    assert_eq!(v["decision"].as_str(), Some("deny"), "{v}");
    Ok(())
}
