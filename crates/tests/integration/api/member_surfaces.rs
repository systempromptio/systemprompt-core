//! The routes the `mcp-http`, `marketplace-http` and `cloud` members'
//! surfaces build, on the real router the API mount layers. Unauthenticated,
//! the MCP executions lookup and the services-tree sync answer 401 at their
//! gates and the managed consumer enrollment answers the consumer contract's
//! 401; with the providing member disabled none of them is mounted and each
//! answers 404.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_extension::reserved::CoreId;
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_models::modules::ApiPaths;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_config};
use tower::ServiceExt;

const EXECUTION: &str = "/api/v1/mcp/executions/exec_missing";
const SYNC_MANIFEST: &str = "/api/v1/sync/files/manifest";

async fn router(disabled: &[ExtensionId]) -> Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx = systemprompt_test_fixtures::fixture_app_context_with_extensions(
        config,
        super::common::reference_admitted(disabled)?,
    )
    .await?;
    setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

async fn send(app: &Router, method: Method, uri: &str) -> Result<(StatusCode, String)> {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::USER_AGENT, "member-surfaces-test/1.0")
        .body(Body::empty())?;
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
    let resp = app.clone().oneshot(req).await?;
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024).await?;
    Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
}

#[tokio::test]
async fn the_member_surface_routes_refuse_an_anonymous_caller_at_their_gates() -> Result<()> {
    let app = router(&[]).await?;
    for uri in [EXECUTION, SYNC_MANIFEST] {
        let (status, body) = send(&app, Method::GET, uri).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}: {body}");
    }
    let (status, body) = send(&app, Method::POST, ApiPaths::CONSUMER_DEVICE_ENROLLMENT).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(body.contains("\"status\":401"), "{body}");
    Ok(())
}

#[tokio::test]
async fn disabling_mcp_http_unmounts_the_executions_lookup_and_the_proxy() -> Result<()> {
    let app = router(&[ExtensionId::new(CoreId::McpHttp.id())]).await?;
    let (status, body) = send(&app, Method::GET, EXECUTION).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body) = send(&app, Method::POST, "/api/v1/mcp/any-server/mcp").await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    Ok(())
}

#[tokio::test]
async fn disabling_marketplace_http_unmounts_the_managed_consumer_routes() -> Result<()> {
    let app = router(&[ExtensionId::new(CoreId::MarketplaceHttp.id())]).await?;
    let (status, body) = send(&app, Method::POST, ApiPaths::CONSUMER_DEVICE_ENROLLMENT).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    Ok(())
}

#[tokio::test]
async fn disabling_cloud_unmounts_the_sync_routes() -> Result<()> {
    let app = router(&[ExtensionId::new(CoreId::Cloud.id())]).await?;
    let (status, body) = send(&app, Method::GET, SYNC_MANIFEST).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    Ok(())
}
