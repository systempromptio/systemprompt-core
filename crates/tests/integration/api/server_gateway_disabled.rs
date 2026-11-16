//! A profile that disables the `gateway` member boots a node that serves no
//! gateway route; with the member enabled the same node mounts them.

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
};
use tower::ServiceExt;

const NO_ROUTE: &str = "No route matches";

async fn node(disabled: &[ExtensionId]) -> anyhow::Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_extensions(
        config,
        super::common::reference_admitted(disabled)?,
    )
    .await?;
    systemprompt_api::services::server::setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &std::sync::Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e:#}"))
}

async fn is_mounted(app: &Router, uri: &str) -> anyhow::Result<bool> {
    let mut req = Request::builder()
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::USER_AGENT, "gateway-disabled-test/1.0")
        .body(Body::empty())?;
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
    let resp = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024).await?;
    let body = String::from_utf8_lossy(&body);
    Ok(!(status == StatusCode::NOT_FOUND && (body.is_empty() || body.contains(NO_ROUTE))))
}

#[tokio::test]
async fn an_enabled_gateway_member_mounts_the_gateway_routes() -> anyhow::Result<()> {
    let app = node(&[]).await?;
    assert!(is_mounted(&app, "/v1/models").await?);
    Ok(())
}

#[tokio::test]
async fn a_disabled_gateway_member_boots_and_serves_no_gateway_route() -> anyhow::Result<()> {
    let app = node(&[ExtensionId::new("gateway")]).await?;
    assert!(!is_mounted(&app, "/v1/models").await?);
    assert!(!is_mounted(&app, "/v1/messages").await?);
    Ok(())
}
