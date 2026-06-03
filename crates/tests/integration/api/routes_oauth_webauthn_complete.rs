//! `/webauthn/complete` — `handle_webauthn_complete`. This bridges a verified
//! WebAuthn authentication token into an authorization code. Without a real
//! verified-authentication token in the WebAuthn registry, every request
//! resolves to a deterministic error: a missing `auth_token` is
//! `invalid_request`, and an unverifiable token is `access_denied`. We drive
//! the query-parameter validation and error mappings; the success path
//! requires a live WebAuthn ceremony (flagged).

use std::sync::{Arc, Once};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::Response;
use systemprompt_api::routes::oauth::public_router;
use systemprompt_models::Config;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use systemprompt_oauth::OAuthState;
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;

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
            jwt_issuer: "test".to_owned(),
            jwt_access_token_expiration: 3600,
            jwt_refresh_token_expiration: 86_400,
            jwt_audiences: vec![],
            allowed_resource_audiences: vec!["hook".to_owned()],
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

async fn webauthn_app() -> anyhow::Result<Router> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    Ok(public_router().with_state(state))
}

async fn read_json(resp: Response<Body>) -> anyhow::Result<serde_json::Value> {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn webauthn_complete_missing_user_id_returns_4xx() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    // `user_id` is a required query field; the Query extractor rejects the
    // request before the handler runs.
    let resp = app.oneshot(empty_get("/webauthn/complete")).await?;
    assert!(resp.status().is_client_error(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn webauthn_complete_missing_auth_token_returns_invalid_request() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let resp = app
        .oneshot(empty_get("/webauthn/complete?user_id=some-user"))
        .await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(
        v["error"].as_str(),
        Some("invalid_request"),
        "missing auth_token must be invalid_request, got {v}"
    );
    Ok(())
}

#[tokio::test]
async fn webauthn_complete_unverifiable_token_is_error() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let resp = app
        .oneshot(empty_get(
            "/webauthn/complete?user_id=some-user&auth_token=not-a-real-token",
        ))
        .await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    // An unconsumable auth token never yields a success; the fixture context
    // has no usable WebAuthn relying-party config, so the route surfaces an
    // OAuth-shaped error (access_denied, or server_error if init fails first).
    assert!(
        status.is_client_error() || status.is_server_error(),
        "{status} {v}"
    );
    assert!(
        v.get("error").and_then(|e| e.as_str()).is_some(),
        "missing error field: {v}"
    );
    Ok(())
}

#[tokio::test]
async fn webauthn_complete_with_full_query_runs_handler() -> anyhow::Result<()> {
    let app = webauthn_app().await?;
    let resp = app
        .oneshot(empty_get(
            "/webauthn/complete?user_id=u&auth_token=tok&client_id=c&redirect_uri=http%3A%2F%2Flocalhost%2Fcb&scope=openid&state=st&code_challenge=ch&code_challenge_method=S256",
        ))
        .await?;
    let status = resp.status().as_u16();
    // The token is still unverifiable, so this lands on access_denied; the
    // point is to drive query deserialisation of every optional field.
    assert!((400..600).contains(&status), "{status}");
    Ok(())
}
