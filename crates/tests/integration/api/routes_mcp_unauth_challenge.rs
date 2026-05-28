//! Regression guard: unauthenticated requests to `/api/v1/mcp/*` must NOT be
//! rejected with the generic `caller type 'anon' is not authorized for this
//! route` 403 emitted by `authz_gate`.
//!
//! v0.11.0 mounted `/api/v1/mcp/*` under `AuthzPolicy::restricted_to([User,
//! Admin, Mcp, Service])`, which sits ABOVE the proxy handler that already
//! handled auth correctly and emits a spec-compliant 401 +
//! `WWW-Authenticate: Bearer resource_metadata="…"` (RFC 9728). The added gate
//! short-circuited the request before the proxy could run, collapsing the
//! response to a generic 403 and breaking every spec-compliant MCP client
//! (Cowork, Claude Code, etc.) — they only start their OAuth discovery dance
//! on a 401 carrying the challenge.
//!
//! This test drives the FULL mounted router (the same one `setup_api_server`
//! ships in production), POSTs to an MCP path with no `Authorization` header,
//! and asserts the response is NOT the regression body. It would have caught
//! the v0.11.0 regression at CI time. Pair with the unit test
//! `unit/entry/api/src/middleware/authz_policy.rs` (gate-level coverage).

use std::sync::{Arc, OnceLock};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use http::Method;
use systemprompt_analytics::{AnalyticsService, FingerprintRepository};
use systemprompt_api::services::server::setup_api_server;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::profile::PathsConfig;
use systemprompt_models::{AppPaths, RouteClassifier};
use systemprompt_runtime::{AppContext, AppContextParts, ModuleApiRegistry};
use systemprompt_security::authz::{AllowAllHook, NullAuditSink};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_config, fixture_db_pool, fixture_system_admin, fixture_user_id,
};
use systemprompt_users::UserService;
use tower::ServiceExt;

async fn boot_full_router() -> anyhow::Result<axum::Router> {
    let bootstrap = ensure_test_bootstrap();
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
    let app_paths = Arc::new(AppPaths::from_profile(&paths)?);

    let parts = AppContextParts {
        config: Arc::new(config),
        database: Arc::clone(&pool),
        api_registry: Arc::new(ModuleApiRegistry::new()),
        extension_registry: Arc::new(ExtensionRegistry::new()),
        geoip_reader: None,
        content_config: None,
        route_classifier: Arc::new(RouteClassifier::new(None)),
        analytics_service: Arc::new(AnalyticsService::new(&pool, None, None)?),
        fingerprint_repo: Some(Arc::new(FingerprintRepository::new(&pool)?)),
        user_service: Some(Arc::new(UserService::new(&pool)?)),
        app_paths,
        marketplace_filter: Arc::new(AllowAllFilter),
        event_bridge: Arc::new(OnceLock::new()),
        system_admin: Arc::new(fixture_system_admin("admin")),
        mcp_registry: RegistryService::new(fixture_user_id()),
        authz_hook: Arc::new(AllowAllHook::new(Arc::new(NullAuditSink))),
    };
    let ctx = Arc::new(AppContext::from_parts(parts));
    let server = setup_api_server(&ctx, None)
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    Ok(server.into_router())
}

async fn body_text(resp: axum::http::Response<Body>) -> (StatusCode, String) {
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 64 * 1024)
        .await
        .expect("body collect");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// Core regression assertion. The response body must NOT be the
/// `authz_gate` anon-denial — that body only appears when a route's
/// `with_auth(..., AuthzPolicy::...)` policy excludes `UserType::Anon`. The
/// MCP route must defer that decision to the proxy handler so an RFC 9728
/// challenge can be emitted instead.
#[tokio::test]
async fn unauthenticated_mcp_post_does_not_hit_generic_anon_403() -> anyhow::Result<()> {
    let app = boot_full_router().await?;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/mcp/__regression__/mcp")
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        ))?;
    let resp = app.oneshot(req).await?;
    let (status, body) = body_text(resp).await;

    assert!(
        !body.contains("'anon' is not authorized for this route"),
        "MCP route mount regressed to a gate that excludes Anon. The proxy handler \
         (services/proxy/auth.rs) cannot emit its RFC 9728 401 challenge if the gate 403s first. \
         status={status} body={body}"
    );
    Ok(())
}

/// RFC 6750 §3 / RFC 9728: when the client sends NO `Authorization` header,
/// the 401 challenge must be the spec-compliant "no credentials" form — bare
/// `WWW-Authenticate: Bearer realm="…", resource_metadata="…"` with no
/// `error=` parameter, and an empty JSON body. Cowork / Claude Code only start
/// the OAuth discovery dance on this exact shape; an `error="invalid_token"`
/// on the same response makes the client report "token rejected" and abort.
/// This pins the 12.x fix and would have caught the 11.0–12.2 regression.
#[tokio::test]
async fn unauthenticated_mcp_post_emits_clean_no_credentials_challenge() -> anyhow::Result<()> {
    let app = boot_full_router().await?;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/mcp/__regression__/mcp")
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        ))?;
    let resp = app.oneshot(req).await?;
    let status = resp.status();
    let www_auth = resp
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .map(|v| v.to_str().unwrap_or_default().to_owned());
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await?;
    let body = String::from_utf8_lossy(&bytes).into_owned();

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "expected 401 challenge, got {status} body={body}"
    );
    let www_auth = www_auth.expect("WWW-Authenticate header present on 401");
    assert!(
        www_auth.starts_with("Bearer "),
        "WWW-Authenticate must be a Bearer challenge, got: {www_auth}"
    );
    assert!(
        www_auth.contains("resource_metadata="),
        "WWW-Authenticate must carry resource_metadata link (RFC 9728), got: {www_auth}"
    );
    assert!(
        !www_auth.contains("error="),
        "RFC 6750 §3 violation: no-credentials challenge must omit error= so clients begin OAuth \
         discovery rather than treat the request as rejected. WWW-Authenticate={www_auth}"
    );
    assert!(
        !body.contains("\"error\""),
        "no-credentials 401 body must not advertise an OAuth error code; got: {body}"
    );
    assert!(
        !body.contains("authorization_url"),
        "non-standard `authorization_url` field must not appear in the 401 body — authorization \
         servers are advertised via the `authorization_servers` array in oauth-protected-resource \
         metadata. body={body}"
    );
    Ok(())
}

/// Companion to the above: when the client DOES present a bearer token and it
/// is rejected, the challenge must carry `error="invalid_token"` in both the
/// `WWW-Authenticate` header and the JSON body. This pins the other branch of
/// the 12.x split so future refactors can't collapse both cases back together.
#[tokio::test]
async fn mcp_post_with_bad_bearer_emits_invalid_token_challenge() -> anyhow::Result<()> {
    let app = boot_full_router().await?;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/mcp/__regression__/mcp")
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, "Bearer not-a-real-token")
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        ))?;
    let resp = app.oneshot(req).await?;
    let status = resp.status();
    let www_auth = resp
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .map(|v| v.to_str().unwrap_or_default().to_owned());
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await?;
    let body = String::from_utf8_lossy(&bytes).into_owned();

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "expected 401, got {status} body={body}"
    );
    let www_auth = www_auth.expect("WWW-Authenticate header present on 401");
    assert!(
        www_auth.contains("error=\"invalid_token\""),
        "bad-token challenge must advertise error=invalid_token per RFC 6750 §3.1, got: {www_auth}"
    );
    assert!(
        www_auth.contains("resource_metadata="),
        "WWW-Authenticate must still carry resource_metadata for client recovery, got: {www_auth}"
    );
    assert!(
        body.contains("\"error\":\"invalid_token\""),
        "bad-token 401 body must include OAuth error code, got: {body}"
    );
    assert!(
        !body.contains("authorization_url"),
        "non-standard `authorization_url` field must not appear in the 401 body; body={body}"
    );
    Ok(())
}

/// Companion assertion: the same unauthenticated GET to a `/mcp` sub-path
/// must also bypass the generic anon-denial. Catches re-introduction of the
/// gate at any HTTP method on the MCP nest.
#[tokio::test]
async fn unauthenticated_mcp_get_does_not_hit_generic_anon_403() -> anyhow::Result<()> {
    let app = boot_full_router().await?;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/mcp/__regression__/mcp")
        .header(header::HOST, "127.0.0.1")
        .body(Body::empty())?;
    let resp = app.oneshot(req).await?;
    let (status, body) = body_text(resp).await;
    assert!(
        !body.contains("'anon' is not authorized for this route"),
        "MCP GET regressed to anon-denial gate. status={status} body={body}"
    );
    Ok(())
}

/// Sibling MCP-registry route (`/api/v1/mcp-registry`) is intentionally
/// public — also assert it stays open to unauthenticated callers so we don't
/// regress the discovery surface at the same time.
#[tokio::test]
async fn unauthenticated_mcp_registry_get_is_not_anon_denied() -> anyhow::Result<()> {
    let app = boot_full_router().await?;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/mcp-registry")
        .header(header::HOST, "127.0.0.1")
        .body(Body::empty())?;
    let resp = app.oneshot(req).await?;
    let (status, body) = body_text(resp).await;
    assert!(
        !body.contains("'anon' is not authorized for this route"),
        "mcp-registry regressed to anon-denial gate. status={status} body={body}"
    );
    Ok(())
}
