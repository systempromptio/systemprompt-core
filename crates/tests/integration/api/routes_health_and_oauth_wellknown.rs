//! `handle_health` and oauth `wellknown_routes` — these handlers query the
//! DB and depend on `Config`, so we mount them against the fixture context.

use axum::Router;
use axum::routing::get;
use std::sync::Once;
use systemprompt_api::routes::oauth::wellknown_routes;
use systemprompt_models::Config;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use tower::ServiceExt;

use super::common::{empty_get, setup_ctx};

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
        id_jag_ttl_secs: 300,
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

#[tokio::test]
async fn handle_health_returns_status() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app: Router = Router::new()
        .route(
            "/health",
            get(systemprompt_api::services::server::builder::handle_health),
        )
        .with_state((*ctx).clone());
    let resp = app.oneshot(empty_get("/health")).await?;
    let status = resp.status().as_u16();
    assert!(status == 200 || status == 503, "{status}");
    Ok(())
}

#[tokio::test]
async fn oauth_wellknown_routes_oauth_authz_server() -> anyhow::Result<()> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_routes(&ctx);
    let resp = app
        .oneshot(empty_get("/.well-known/oauth-authorization-server"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn oauth_wellknown_routes_openid_config() -> anyhow::Result<()> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_routes(&ctx);
    let resp = app
        .oneshot(empty_get("/.well-known/openid-configuration"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn oauth_wellknown_routes_protected_resource() -> anyhow::Result<()> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_routes(&ctx);
    let resp = app
        .oneshot(empty_get("/.well-known/oauth-protected-resource"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn oauth_wellknown_routes_protected_resource_with_path() -> anyhow::Result<()> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_routes(&ctx);
    let resp = app
        .oneshot(empty_get(
            "/.well-known/oauth-protected-resource/mcp/foo/mcp",
        ))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}
