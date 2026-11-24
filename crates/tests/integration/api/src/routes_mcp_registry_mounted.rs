//! The MCP registry route on the real router, mounted from the `mcp-http`
//! extension's declaration:
//!
//! | path | anonymous | `mcp-http` disabled |
//! |---|---|---|
//! | `GET /api/v1/mcp/registry` | served | 404 |
//!
//! The set holds that one path: a path below it or beside it under
//! `/api/v1/mcp` is the kernel MCP proxy's, which refuses an anonymous call.
//! The `All`, `Api` and `Worker` roles serve the route, and a profile that
//! disables `mcp` while `mcp-http` stays enabled is refused at boot.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_extension::reserved::CoreId;
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
};
use tower::ServiceExt;

const REGISTRY: &str = "/api/v1/mcp/registry";

async fn node(role: NodeRole, disabled: &[ExtensionId]) -> Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let mut config = fixture_config(&bootstrap.database_url);
    config.role = role;
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx =
        fixture_app_context_with_extensions(config, super::common::reference_admitted(disabled)?)
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

async fn get(app: &Router, uri: &str) -> Result<(StatusCode, String)> {
    let mut req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .body(Body::empty())?;
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
    let response = app.clone().oneshot(req).await?;
    let status = response.status();
    let body = response.into_body().collect().await?.to_bytes();
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

#[tokio::test]
async fn the_registry_serves_an_anonymous_caller() -> Result<()> {
    let app = node(NodeRole::All, &[]).await?;
    let (status, body) = get(&app, REGISTRY).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listing: serde_json::Value = serde_json::from_str(&body)?;
    assert!(listing["data"].is_array(), "{listing}");
    Ok(())
}

#[tokio::test]
async fn a_path_below_the_registry_is_not_answered_by_the_registry() -> Result<()> {
    let app = node(NodeRole::All, &[]).await?;
    for uri in ["/api/v1/mcp/registry/x", "/api/v1/mcp/registryx/mcp"] {
        let (status, body) = get(&app, uri).await?;
        assert_ne!(status, StatusCode::OK, "{uri}: {body}");
        assert!(
            serde_json::from_str::<serde_json::Value>(&body)
                .map_or(true, |value| !value["data"].is_array()),
            "{uri} answered a registry listing: {body}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn disabling_mcp_http_unmounts_the_registry() -> Result<()> {
    let app = node(NodeRole::All, &[ExtensionId::new(CoreId::McpHttp.id())]).await?;
    let (status, body) = get(&app, REGISTRY).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    Ok(())
}

#[tokio::test]
async fn disabling_mcp_while_the_registry_stays_enabled_is_refused_at_boot() -> Result<()> {
    let refused = node(NodeRole::All, &[ExtensionId::new("mcp")]).await;
    let message = match refused {
        Ok(_) => anyhow::bail!("boot accepted mcp-http without the mcp extension"),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        message.contains("its core provider mcp is disabled"),
        "{message}"
    );
    Ok(())
}

#[tokio::test]
async fn the_all_api_and_worker_roles_serve_the_registry() -> Result<()> {
    for role in [NodeRole::All, NodeRole::Api, NodeRole::Worker] {
        let app = node(role, &[]).await?;
        assert_eq!(get(&app, REGISTRY).await?.0, StatusCode::OK, "{role:?}");
    }
    for role in [NodeRole::Gateway, NodeRole::Runner] {
        let app = node(role, &[]).await?;
        assert_eq!(
            get(&app, REGISTRY).await?.0,
            StatusCode::NOT_FOUND,
            "{role:?}"
        );
    }
    Ok(())
}
