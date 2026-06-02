//! `/callback` OAuth callback endpoint — `handle_callback`. The browser-flow
//! callback exchanges a returned code for tokens, but in the fixture
//! environment there is no provisioned browser client and no valid code, so
//! every path short-circuits to a deterministic error before touching a live
//! dependency. We assert the error mapping for the no-browser-client,
//! bad-code, and missing-state branches.

use std::sync::{Arc, Once};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response};
use axum::middleware::{self, Next};
use systemprompt_api::routes::oauth::public_router;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_models::Config;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::execution::context::RequestContext;
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

async fn callback_app() -> anyhow::Result<Router> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    Ok(public_router()
        .layer(middleware::from_fn(inject_context))
        .with_state(state))
}

#[tokio::test]
async fn callback_missing_code_returns_4xx() -> anyhow::Result<()> {
    let app = callback_app().await?;
    let resp = app.oneshot(empty_get("/callback")).await?;
    // The `code` query field is required; axum's Query extractor rejects the
    // request before the handler runs.
    assert!(resp.status().is_client_error(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn callback_with_code_no_browser_client_returns_5xx() -> anyhow::Result<()> {
    let app = callback_app().await?;
    let resp = app
        .oneshot(empty_get("/callback?code=some-code&state=abc"))
        .await?;
    // No browser client is provisioned in the fixture DB, so
    // `find_browser_client` fails and the handler returns 500.
    assert!(
        resp.status().is_server_error(),
        "expected 5xx for missing browser client, got {}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn callback_with_code_only_runs_handler() -> anyhow::Result<()> {
    let app = callback_app().await?;
    let resp = app.oneshot(empty_get("/callback?code=xyz")).await?;
    let status = resp.status().as_u16();
    assert!((400..600).contains(&status), "{status}");
    Ok(())
}

#[tokio::test]
async fn callback_empty_code_value_runs_handler() -> anyhow::Result<()> {
    let app = callback_app().await?;
    let resp = app.oneshot(empty_get("/callback?code=&state=s")).await?;
    let status = resp.status().as_u16();
    assert!((400..600).contains(&status), "{status}");
    Ok(())
}
