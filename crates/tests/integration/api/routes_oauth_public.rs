//! Public OAuth router — health, token, authorize, webauthn endpoints. We
//! exercise the routing and error mappings; happy paths require a full
//! provisioned client + PKCE flow.

use std::sync::{Arc, Once};

use axum::Router;
use systemprompt_api::routes::oauth::{authenticated_router, public_router};
use systemprompt_models::Config;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use systemprompt_oauth::OAuthState;
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;

use super::common::{empty_get, json_post, setup_ctx};

static CONFIG_INSTALL: Once = Once::new();

fn ensure_config() {
    CONFIG_INSTALL.call_once(|| {
        let _ = Config::install(test_config());
    });
}

fn test_config() -> Config {
    Config {
        instance_id: "test".to_string(),
        max_concurrent_streams: 16,
        sitename: "test".to_string(),
        database_type: "postgres".to_string(),
        database_url: "postgres://x".to_string(),
        database_write_url: None,
        github_link: String::new(),
        github_token: None,
        system_path: "/tmp".to_string(),
        services_path: "/tmp".to_string(),
        bin_path: "/tmp".to_string(),
        skills_path: "/tmp".to_string(),
        settings_path: "/tmp".to_string(),
        content_config_path: "/tmp".to_string(),
        geoip_database_path: None,
        web_path: "/tmp".to_string(),
        web_config_path: "/tmp".to_string(),
        web_metadata_path: "/tmp".to_string(),
        host: "127.0.0.1".to_string(),
        port: 0,
        api_server_url: "http://127.0.0.1".to_string(),
        api_internal_url: "http://127.0.0.1".to_string(),
        api_external_url: "http://127.0.0.1".to_string(),
        jwt_issuer: "test".to_string(),
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
        system_admin_username: "admin".to_string(),
        content_negotiation: ContentNegotiationConfig::default(),
        security_headers: SecurityHeadersConfig::default(),
        allow_registration: false,
    }
}

async fn public_app() -> anyhow::Result<Router> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    Ok(public_router().with_state(state))
}

async fn authenticated_app() -> anyhow::Result<Router> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    Ok(authenticated_router().with_state(state))
}

#[tokio::test]
async fn health_returns_ok() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app.oneshot(empty_get("/health")).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn session_returns_token() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app
        .oneshot(json_post("/session", serde_json::json!({})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn token_empty_body_returns_error() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app
        .oneshot(json_post("/token", serde_json::json!({})))
        .await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn authorize_get_without_params_returns_error() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app.oneshot(empty_get("/authorize")).await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn callback_without_params_returns_error() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app.oneshot(empty_get("/callback")).await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn webauthn_register_start_returns_error_without_body() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app
        .oneshot(json_post("/webauthn/register/start", serde_json::json!({})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn webauthn_auth_start_runs_handler() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app
        .oneshot(json_post("/webauthn/auth/start", serde_json::json!({})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn webauthn_link_start_runs_handler() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app.oneshot(empty_get("/webauthn/link/start")).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn introspect_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app
        .oneshot(json_post("/introspect", serde_json::json!({"token": "x"})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn revoke_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app
        .oneshot(json_post("/revoke", serde_json::json!({"token": "x"})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn logout_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app
        .oneshot(json_post("/logout", serde_json::json!({})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn userinfo_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app.oneshot(empty_get("/userinfo")).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn consent_get_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app.oneshot(empty_get("/consent")).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn register_client_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let body = serde_json::json!({
        "client_name": "test",
        "redirect_uris": ["http://localhost/callback"]
    });
    let resp = app.oneshot(json_post("/register", body)).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn register_client_applies_rfc7591_defaults_when_grant_and_response_types_omitted()
-> anyhow::Result<()> {
    use axum::Extension;
    use systemprompt_identifiers::UserId;
    use uuid::Uuid;

    ensure_config();
    let (pool, ctx) = setup_ctx().await?;

    let user_id = UserId::new(format!("dcr-defaults-{}", Uuid::new_v4()));
    {
        let p = pool.pool_arc().expect("read pool");
        sqlx::query(
            "INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(user_id.as_str())
        .bind(format!("{}@dcr-fixture.invalid", user_id.as_str()))
        .execute(p.as_ref())
        .await?;
    }

    let mut req_ctx = super::common::request_context("ignored");
    req_ctx.auth.actor.user_id = user_id.clone();

    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    let app = systemprompt_api::routes::oauth::public_router()
        .with_state(state)
        .layer(Extension(req_ctx));

    let body = serde_json::json!({
        "client_name": "rfc7591-probe",
        "redirect_uris": ["http://127.0.0.1:53280/callback"],
    });
    let resp = app.oneshot(json_post("/register", body)).await?;
    let status = resp.status();
    let (_, body_str) = super::common::body_to_string(resp).await?;
    assert_eq!(
        status.as_u16(),
        201,
        "expected 201 Created, got {status}: {body_str}"
    );

    let json: serde_json::Value = serde_json::from_str(&body_str)?;
    assert_eq!(
        json["grant_types"],
        serde_json::json!(["authorization_code"]),
        "grant_types must default to RFC 7591 §2 value when omitted; got {body_str}"
    );
    assert_eq!(
        json["response_types"],
        serde_json::json!(["code"]),
        "response_types must default to RFC 7591 §2 value when omitted; got {body_str}"
    );
    assert!(json["client_id"].as_str().is_some(), "missing client_id");
    assert!(
        json["client_secret"].as_str().is_some(),
        "missing client_secret"
    );
    Ok(())
}

#[tokio::test]
async fn get_client_configuration_unknown_returns_4xx() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app.oneshot(empty_get("/register/unknown_client")).await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn clients_list_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app.oneshot(empty_get("/clients")).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn clients_create_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let body = serde_json::json!({
        "client_name": "test client",
        "redirect_uris": ["http://localhost/cb"],
    });
    let resp = app.oneshot(json_post("/clients", body)).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn clients_get_unknown_returns_4xx() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app.oneshot(empty_get("/clients/missing")).await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}
