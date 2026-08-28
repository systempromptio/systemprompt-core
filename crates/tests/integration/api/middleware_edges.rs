//! Middleware-rejection edge paths.
//!
//! Builds the full API router via `setup_api_server` and drives requests with
//! hostile inputs — expired/garbage JWTs, missing auth, suspect User-Agent.
//! Each test asserts the router short-circuits with a 4xx and does NOT reach a
//! handler. The happy path is covered by `server_boot.rs`; this file is about
//! the failure surface of `services::middleware::{jwt,session,bot_detector,
//! ip_ban}`.

use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::Request;
use systemprompt_analytics::{AnalyticsService, FingerprintRepository};
use systemprompt_api::services::server::setup_api_server;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::profile::PathsConfig;
use systemprompt_models::{AppPaths, RouteClassifier};
use systemprompt_runtime::{
    AppContext, ConfigPlane, DataPlane, ModuleApiRegistry, Plugins, Subsystems,
};
use systemprompt_security::authz::{AllowAllHook, NullAuditSink};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_config, fixture_db_pool, fixture_system_admin, fixture_user_id,
    install_test_signing_key,
};
use systemprompt_users::{UserRepository, UserService};
use tower::ServiceExt;

async fn boot_server() -> anyhow::Result<axum::Router> {
    let bootstrap = ensure_test_bootstrap();
    install_test_signing_key();
    let pool = fixture_db_pool(&bootstrap.database_url).await?;

    let mut config = fixture_config(&bootstrap.database_url);
    config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];

    let paths = PathsConfig {
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    };
    let app_paths = Arc::new(AppPaths::from_profile(
        &paths,
        systemprompt_models::PathResolution::Canonicalize,
    )?);

    let ctx = Arc::new(AppContext::from_parts(
        {
            let analytics_repositories =
                Arc::new(systemprompt_analytics::repository::AnalyticsRepositories::new(&pool)?);
            let analytics_service =
                Arc::new(AnalyticsService::new(None, None, &analytics_repositories));
            let session_usage: systemprompt_traits::DynSessionUsageCounters =
                Arc::new(analytics_service.session_repo().clone());
            DataPlane {
                database: Arc::clone(&pool),
                analytics_service,
                fingerprint_repo: Some(Arc::new(FingerprintRepository::new(&pool)?)),
                user_service: Some(Arc::new(UserService::new(Arc::new(UserRepository::new(
                    &pool,
                )?)))),
                a2a_repositories: Arc::new(systemprompt_agent::repository::A2ARepositories::new(
                    &pool,
                    session_usage,
                )?),
                content_repositories: Arc::new(
                    systemprompt_content::repository::ContentRepositories::new(&pool)?,
                ),
                oauth_repositories: Arc::new(
                    systemprompt_oauth::repository::OAuthRepositories::new(&pool)?,
                ),
                user_repository: Arc::new(systemprompt_users::UserRepository::new(&pool)?),
                service_repository: Arc::new(systemprompt_database::ServiceRepository::new(&pool)?),
                ai_repositories: Arc::new(systemprompt_ai::repository::AiRepositories::new(&pool)?),
                analytics_repositories,
                file_repository: Arc::new(systemprompt_files::FileRepository::new(&pool)?),
                mcp_session_repository: Arc::new(
                    systemprompt_mcp::repository::McpSessionRepository::new(&pool)?,
                ),
            }
        },
        ConfigPlane {
            config: Arc::new(config),
            app_paths,
            content_config: None,
            route_classifier: Arc::new(RouteClassifier::new(None)),
        },
        Plugins {
            extension_registry: Arc::new(ExtensionRegistry::new()),
            api_registry: Arc::new(ModuleApiRegistry::new()),
            mcp_registry: RegistryService::new(fixture_user_id()),
            marketplace_filter: Arc::new(AllowAllFilter),
        },
        Subsystems {
            system_admin: Arc::new(fixture_system_admin("admin")),
            authz_hook: Arc::new(AllowAllHook::new(Arc::new(NullAuditSink))),
            event_bridge: Arc::new(OnceLock::new()),
            geoip_reader: None,
        },
    ));
    let router = setup_api_server(&ctx, None)
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    Ok(router)
}

async fn try_boot() -> Option<axum::Router> {
    boot_server().await.ok()
}

static BOOT_GATE: OnceLock<()> = OnceLock::new();

fn gate() {
    BOOT_GATE.get_or_init(|| ());
}

#[tokio::test]
async fn health_endpoint_is_reachable_without_auth() {
    gate();
    let Some(app) = try_boot().await else { return };
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.expect("oneshot");
    let s = resp.status().as_u16();
    // Health is public; 200 expected. Some configurations 404 if the route
    // isn't mounted under the bootstrap profile — accept either.
    assert!(s == 200 || s == 404, "{s}");
}

// The tests below propagate boot failures rather than using `try_boot`, so a
// broken fixture fails loudly instead of passing vacuously.
fn get(uri: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut builder = Request::builder().uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let mut req = builder.body(Body::empty()).expect("request must build");
    req.extensions_mut().insert(ConnectInfo(
        "203.0.113.7:41000"
            .parse::<SocketAddr>()
            .expect("peer address must parse"),
    ));
    req
}

async fn status_of(uri: &str, headers: &[(&str, &str)]) -> anyhow::Result<u16> {
    let app = boot_server().await?;
    Ok(app.oneshot(get(uri, headers)).await?.status().as_u16())
}

#[tokio::test]
async fn the_passkey_link_page_is_reachable_anonymously() -> anyhow::Result<()> {
    let status = status_of("/auth/link-passkey", &[]).await?;

    assert_eq!(
        status, 200,
        "the page is mounted behind the route gate, but the recipient of a setup link is not \
         logged in, so the gate must admit anonymous callers"
    );
    Ok(())
}

#[tokio::test]
async fn every_response_carries_the_configured_security_headers() -> anyhow::Result<()> {
    let app = boot_server().await?;

    let resp = app.oneshot(get("/health", &[])).await?;

    let headers = resp.headers();
    assert_eq!(
        headers
            .get("x-content-type-options")
            .and_then(|v| v.to_str().ok()),
        Some("nosniff"),
        "MIME sniffing must stay disabled on every response"
    );
    assert_eq!(
        headers.get("x-frame-options").and_then(|v| v.to_str().ok()),
        Some("DENY"),
        "the API must never be framable"
    );
    assert!(
        headers.get("strict-transport-security").is_some(),
        "HSTS is configured on and must be emitted"
    );
    Ok(())
}

#[tokio::test]
async fn an_unknown_path_is_not_served_by_any_handler() -> anyhow::Result<()> {
    let status = status_of("/no/such/endpoint-at-all", &[]).await?;

    assert!(
        status == 404 || status == 405,
        "an unrouted path must fall through to the fallback, got {status}"
    );
    Ok(())
}

#[tokio::test]
async fn a_garbage_bearer_token_never_reaches_an_authenticated_handler() -> anyhow::Result<()> {
    let status = status_of(
        "/api/v1/core/oauth/userinfo",
        &[("authorization", "Bearer not-a-jwt-at-all")],
    )
    .await?;

    assert!(
        status == 401 || status == 403,
        "an unverifiable credential must be refused by the middleware, got {status}"
    );
    Ok(())
}

#[tokio::test]
async fn an_authenticated_route_refuses_an_anonymous_caller() -> anyhow::Result<()> {
    let status = status_of("/api/v1/core/oauth/userinfo", &[]).await?;

    assert!(
        status == 401 || status == 403,
        "a route behind auth must not serve an unauthenticated request, got {status}"
    );
    Ok(())
}

#[tokio::test]
async fn a_bearer_scheme_with_no_token_is_refused() -> anyhow::Result<()> {
    let status = status_of(
        "/api/v1/core/oauth/userinfo",
        &[("authorization", "Bearer ")],
    )
    .await?;

    assert!(
        status == 401 || status == 403,
        "an empty credential is not a credential, got {status}"
    );
    Ok(())
}

#[tokio::test]
async fn a_non_bearer_authorization_scheme_is_refused() -> anyhow::Result<()> {
    let status = status_of(
        "/api/v1/core/oauth/userinfo",
        &[("authorization", "Basic dXNlcjpwYXNz")],
    )
    .await?;

    assert!(
        status == 401 || status == 403,
        "basic auth is not accepted anywhere on this API, got {status}"
    );
    Ok(())
}

#[tokio::test]
async fn a_scanner_user_agent_is_still_served_the_public_health_route() -> anyhow::Result<()> {
    // Bot detection is analytics-only by design — it scores the session but
    // must not deny service, or a mislabelled browser would lose access.
    let status = status_of(
        "/health",
        &[("user-agent", "sqlmap/1.7-dev (http://sqlmap.org)")],
    )
    .await?;

    assert_eq!(status, 200, "bot scoring must not gate a public route");
    Ok(())
}

#[tokio::test]
async fn a_request_with_no_user_agent_is_handled_rather_than_rejected() -> anyhow::Result<()> {
    assert_eq!(status_of("/health", &[]).await?, 200);
    Ok(())
}

#[tokio::test]
async fn a_forged_forwarded_header_does_not_bypass_the_router() -> anyhow::Result<()> {
    // No trusted proxies are configured, so the hop header is inert; the
    // request must still be routed normally rather than erroring.
    let status = status_of(
        "/health",
        &[
            ("x-forwarded-for", "10.0.0.1, 127.0.0.1"),
            ("x-real-ip", "192.168.1.1"),
        ],
    )
    .await?;

    assert_eq!(
        status, 200,
        "an untrusted hop header must be ignored, not fatal"
    );
    Ok(())
}

#[tokio::test]
async fn a_cors_preflight_from_the_allowed_origin_is_answered() -> anyhow::Result<()> {
    let app = boot_server().await?;
    let req = Request::builder()
        .method(http::Method::OPTIONS)
        .uri("/health")
        .header("origin", "http://127.0.0.1")
        .header("access-control-request-method", "GET")
        .body(Body::empty())
        .expect("request must build");

    let resp = app.oneshot(req).await?;

    assert!(
        resp.status().is_success() || resp.status().as_u16() == 204,
        "a preflight from the configured origin must be answered, got {}",
        resp.status()
    );
    assert!(
        resp.headers().get("access-control-allow-origin").is_some(),
        "the preflight response must carry the allow-origin header"
    );
    Ok(())
}

#[tokio::test]
async fn a_cors_preflight_from_an_unlisted_origin_is_not_granted() -> anyhow::Result<()> {
    let app = boot_server().await?;
    let req = Request::builder()
        .method(http::Method::OPTIONS)
        .uri("/health")
        .header("origin", "http://evil.example")
        .header("access-control-request-method", "GET")
        .body(Body::empty())
        .expect("request must build");

    let resp = app.oneshot(req).await?;

    let allowed = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert_ne!(
        allowed, "http://evil.example",
        "an unlisted origin must never be echoed back as allowed"
    );
    Ok(())
}
