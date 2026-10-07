//! `server.role` on the real router: a gateway node answers the gateway and
//! its discovery dependencies but not the admin or agent surfaces; an admin
//! node is the mirror image.

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::services::server::routes::configure_routes_for_role;
use systemprompt_api::services::server::setup_api_server;
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_config, fixture_config, test_db_pool,
};
use tower::ServiceExt;

const NO_ROUTE: &str = "No route matches";

async fn router_for(role: NodeRole) -> anyhow::Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    config.role = role;
    let ctx = fixture_app_context_with_config(&pool, config)?;
    setup_api_server(&ctx, None).map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

async fn get(app: &Router, uri: &str) -> anyhow::Result<(StatusCode, String)> {
    let mut req = Request::builder()
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::USER_AGENT, "role-test/1.0")
        .body(Body::empty())?;
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
    let resp = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024).await?;
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

// Why: an unmatched path is answered by a router fallback, either the
// static site's `No route matches` envelope or, on a gateway node that
// mounts no static site, the empty 404 of the nearest nested router. A
// mounted handler that refuses still names its reason in the body.
async fn is_mounted(app: &Router, uri: &str) -> anyhow::Result<bool> {
    let (status, body) = get(app, uri).await?;
    Ok(!(status == StatusCode::NOT_FOUND && (body.is_empty() || body.contains(NO_ROUTE))))
}

#[tokio::test]
async fn gateway_node_serves_the_gateway_and_refuses_admin_surfaces() -> anyhow::Result<()> {
    let app = router_for(NodeRole::Gateway).await?;

    assert!(is_mounted(&app, "/v1/models").await?);
    assert_eq!(get(&app, "/livez").await?.0, StatusCode::OK);
    assert!(is_mounted(&app, "/.well-known/oauth-authorization-server").await?);
    for uri in ["/api/v1/admin/services", "/api/v1/core/contexts"] {
        let (status, body) = get(&app, uri).await?;
        assert!(
            !is_mounted(&app, uri).await?,
            "{uri} must not be served by a gateway node: {status} {body}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn admin_node_refuses_the_gateway_and_serves_admin_surfaces() -> anyhow::Result<()> {
    let app = router_for(NodeRole::Admin).await?;

    assert!(!is_mounted(&app, "/v1/models").await?);
    assert_eq!(
        get(&app, "/api/v1/core/contexts").await?.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(get(&app, "/livez").await?.0, StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn configure_routes_for_role_mounts_the_gateway_regardless_of_the_configured_role()
-> anyhow::Result<()> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::disabled();
    config.role = NodeRole::Admin;
    let ctx = fixture_app_context_with_config(&pool, config)?;

    let gateway = configure_routes_for_role(&ctx, NodeRole::Gateway, None)?;
    assert!(is_mounted(&gateway, "/v1/models").await?);
    Ok(())
}
