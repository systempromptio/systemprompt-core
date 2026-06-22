//! `oauth/discovery` handlers — the `/.well-known/openid-configuration`
//! variants. The handlers read the process-wide `Config`; we install a test
//! Config once at module load.

use axum::Router;
use axum::routing::get;
use std::sync::Once;
use systemprompt_api::routes::oauth;
use systemprompt_models::Config;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use tower::ServiceExt;

use super::common::{body_to_string, empty_get, setup_ctx};

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

async fn discovery_app() -> anyhow::Result<Router> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    Ok(Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(oauth::discovery::handle_well_known),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(oauth::discovery::handle_oauth_protected_resource),
        )
        .route(
            "/.well-known/oauth-protected-resource/{*path}",
            get(oauth::discovery::handle_oauth_protected_resource_with_path),
        )
        .with_state((*ctx).clone()))
}

#[tokio::test]
async fn well_known_returns_openid_config() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get("/.well-known/openid-configuration"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn well_known_advertises_iss_parameter_support() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get("/.well-known/openid-configuration"))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.is_success(), "{status}");
    let json: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(
        json["authorization_response_iss_parameter_supported"],
        serde_json::Value::Bool(true)
    );
    Ok(())
}

#[tokio::test]
async fn protected_resource_returns_metadata() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get("/.well-known/oauth-protected-resource"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn well_known_advertises_token_exchange_and_id_jag() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get("/.well-known/openid-configuration"))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.is_success(), "{status}");
    let json: serde_json::Value = serde_json::from_str(&body)?;

    let grants = json["grant_types_supported"]
        .as_array()
        .expect("grant_types_supported is an array");
    assert!(
        grants
            .iter()
            .any(|g| g == "urn:ietf:params:oauth:grant-type:token-exchange"),
        "token-exchange grant must be advertised: {json}"
    );
    for field in [
        "subject_token_types_supported",
        "issued_token_types_supported",
    ] {
        let types = json[field].as_array().expect("token-type array");
        assert!(
            types
                .iter()
                .any(|t| t == "urn:ietf:params:oauth:token-type:id-jag"),
            "{field} must advertise id-jag: {json}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn protected_resource_with_path_falls_back_to_default() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get(
            "/.well-known/oauth-protected-resource/unrelated/path",
        ))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn protected_resource_with_mcp_path_returns_metadata() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app
        .oneshot(empty_get(
            "/.well-known/oauth-protected-resource/mcp/my-server/mcp",
        ))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}
