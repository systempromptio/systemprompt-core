//! Coverage for the proxy service resolver and the external-MCP outbound
//! header/error mapping helpers.
//!
//! The resolver is seeded with `services` rows per status, so this suite must
//! run in the `scheduler-services-db` serial nextest group. The restart path
//! (`crashed` status) spawns a subprocess and is intentionally not exercised
//! here.

use axum::http::{HeaderName, HeaderValue};
use systemprompt_api::services::proxy::engine_test_api::{map_resolve_error, outbound_headers};
use systemprompt_api::services::proxy::resolver_test_api::resolve;
use systemprompt_database::{CreateServiceInput, ServiceRepository};
use systemprompt_mcp::McpDomainError;
use uuid::Uuid;

use super::common::setup_ctx;

async fn seed(
    ctx: &systemprompt_runtime::AppContext,
    name: &str,
    status: &str,
) -> anyhow::Result<()> {
    let repo = ServiceRepository::new(ctx.db_pool())?;
    repo.create_service(CreateServiceInput {
        name,
        module_name: "mcp",
        status,
        port: 0,
        binary_mtime: None,
    })
    .await?;
    Ok(())
}

#[tokio::test]
async fn resolve_returns_running_service() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let name = format!("run-{}", Uuid::new_v4().simple());
    seed(&ctx, &name, "running").await?;

    let config = resolve(&name, &ctx)
        .await
        .map_err(|e| anyhow::anyhow!("resolve failed: {e}"))?;
    assert_eq!(config.name, name);
    assert_eq!(config.status, "running");
    Ok(())
}

#[tokio::test]
async fn resolve_missing_service_is_not_found() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let err = resolve(&format!("absent-{}", Uuid::new_v4().simple()), &ctx)
        .await
        .expect_err("missing service must error");
    assert!(matches!(
        err,
        systemprompt_api::services::proxy::ProxyError::ServiceNotFound { .. }
    ));
    Ok(())
}

#[tokio::test]
async fn resolve_stopped_service_is_not_running() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let name = format!("stop-{}", Uuid::new_v4().simple());
    seed(&ctx, &name, "stopped").await?;

    let err = resolve(&name, &ctx).await.expect_err("stopped must error");
    assert!(matches!(
        err,
        systemprompt_api::services::proxy::ProxyError::ServiceNotRunning { .. }
    ));
    Ok(())
}

#[test]
fn outbound_headers_forwards_passthrough_and_provider() {
    let mut incoming = axum::http::HeaderMap::new();
    incoming.insert("content-type", HeaderValue::from_static("application/json"));
    incoming.insert("mcp-session-id", HeaderValue::from_static("sess-1"));
    incoming.insert("x-secret", HeaderValue::from_static("nope"));

    let provider = vec![(
        HeaderName::from_static("authorization"),
        HeaderValue::from_static("Bearer provider-token"),
    )];

    let out = outbound_headers(&incoming, provider);
    assert_eq!(out.get("content-type").unwrap(), "application/json");
    assert_eq!(out.get("mcp-session-id").unwrap(), "sess-1");
    assert_eq!(out.get("authorization").unwrap(), "Bearer provider-token");
    assert!(
        out.get("x-secret").is_none(),
        "non-passthrough client headers are dropped"
    );
}

#[test]
fn map_resolve_error_classifies_domain_errors() {
    let auth = map_resolve_error("svc", McpDomainError::AuthRequired("need".to_owned()));
    assert!(auth.contains("Authentication required"));

    let unavailable = map_resolve_error(
        "svc",
        McpDomainError::ExternalAuthUnavailable {
            server: "svc".to_owned(),
            message: "vault down".to_owned(),
        },
    );
    assert!(unavailable.contains("not running") || unavailable.contains("vault down"));
}
