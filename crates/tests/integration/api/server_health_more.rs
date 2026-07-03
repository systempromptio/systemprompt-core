//! Coverage for the health endpoints, the byte/`/proc` formatting helpers, the
//! scheduler-health record, the stale-service reconciliation predicate, and the
//! shutdown child-termination sweep.
//!
//! The reconciliation-cleanup and shutdown tests seed `services` rows with dead
//! or bogus PIDs, so this suite must run in the `scheduler-services-db` serial
//! nextest group to avoid clobbering parallel service-table tests.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::routing::get;
use systemprompt_api::services::server::reconciliation_test_api::{
    cleanup_stale_service_entries, service_row_is_stale,
};
use systemprompt_api::services::server::test_api::{handle_health_detail, human_bytes};
use systemprompt_api::services::server::{handle_health, scheduler_health, shutdown_test_api};
use systemprompt_database::{CreateServiceInput, ServiceRepository};
use systemprompt_models::subprocess::MCP_SERVICE_ID_ENV;
use systemprompt_runtime::AppContext;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::setup_ctx;

fn dead_pid() -> i32 {
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn sleep");
    let pid = child.id() as i32;
    child.kill().expect("kill child");
    child.wait().expect("reap child");
    pid
}

async fn seed_mcp_service(
    ctx: &AppContext,
    name: &str,
    status: &str,
    pid: Option<i32>,
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
    if let Some(pid) = pid {
        repo.update_service_pid(name, pid).await?;
    }
    Ok(())
}

async fn seed_agent_service(
    ctx: &AppContext,
    name: &str,
    status: &str,
    pid: Option<i32>,
) -> anyhow::Result<()> {
    let repo = ServiceRepository::new(ctx.db_pool())?;
    repo.create_service(CreateServiceInput {
        name,
        module_name: "agent",
        status,
        port: 0,
        binary_mtime: None,
    })
    .await?;
    if let Some(pid) = pid {
        repo.update_service_pid(name, pid).await?;
    }
    Ok(())
}

#[test]
fn human_bytes_scales_units() {
    assert_eq!(human_bytes(0), "0.0 B");
    assert_eq!(human_bytes(1024), "1.0 KB");
    assert_eq!(human_bytes(1024 * 1024), "1.0 MB");
    assert_eq!(human_bytes(5 * 1024 * 1024 * 1024), "5.0 GB");
}

#[test]
fn scheduler_health_records() {
    scheduler_health::record(Vec::new());
    assert!(scheduler_health::degraded().is_empty());
}

#[test]
fn service_row_is_stale_across_statuses() {
    assert!(service_row_is_stale(
        "error",
        None,
        MCP_SERVICE_ID_ENV,
        "svc"
    ));
    assert!(service_row_is_stale(
        "stopped",
        Some(1),
        MCP_SERVICE_ID_ENV,
        "svc"
    ));
    assert!(!service_row_is_stale(
        "unknown",
        None,
        MCP_SERVICE_ID_ENV,
        "svc"
    ));
    assert!(
        service_row_is_stale("running", None, MCP_SERVICE_ID_ENV, "svc"),
        "running with no pid is stale"
    );
    assert!(
        service_row_is_stale("running", Some(dead_pid()), MCP_SERVICE_ID_ENV, "svc"),
        "running with a dead pid is stale"
    );
    assert!(
        service_row_is_stale(
            "running",
            Some(std::process::id() as i32),
            MCP_SERVICE_ID_ENV,
            "not-our-child"
        ),
        "a live but unrelated pid is stale (recycled)"
    );
}

#[tokio::test]
async fn cleanup_removes_stale_mcp_rows() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let name = format!("stale-mcp-{}", Uuid::new_v4().simple());
    seed_mcp_service(&ctx, &name, "error", None).await?;

    let deleted = cleanup_stale_service_entries(&ctx).await?;
    assert!(deleted >= 1, "the error-status row must be swept");

    let repo = ServiceRepository::new(ctx.db_pool())?;
    assert!(
        repo.find_service_by_name(&name).await?.is_none(),
        "stale row is gone"
    );
    Ok(())
}

#[tokio::test]
async fn shutdown_drain_clears_dead_and_recycled_children() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let dead = format!("dead-mcp-{}", Uuid::new_v4().simple());
    let recycled = format!("recycled-mcp-{}", Uuid::new_v4().simple());
    seed_mcp_service(&ctx, &dead, "running", Some(dead_pid())).await?;
    seed_mcp_service(&ctx, &recycled, "running", Some(std::process::id() as i32)).await?;

    shutdown_test_api::terminate_children(&ctx).await;

    let repo = ServiceRepository::new(ctx.db_pool())?;
    let recycled_row = repo
        .find_service_by_name(&recycled)
        .await?
        .expect("recycled row still present");
    assert_ne!(
        recycled_row.status, "running",
        "a live non-child pid is cleared, not signalled"
    );

    shutdown_test_api::drain(&ctx).await;
    Ok(())
}

#[tokio::test]
async fn cleanup_sweeps_stale_agent_row_and_keeps_non_stale_mcp() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let stale_agent = format!("stale-agent-{}", Uuid::new_v4().simple());
    let live_mcp = format!("live-mcp-{}", Uuid::new_v4().simple());
    seed_agent_service(&ctx, &stale_agent, "error", None).await?;
    seed_mcp_service(&ctx, &live_mcp, "unknown", None).await?;

    let deleted = cleanup_stale_service_entries(&ctx).await?;
    assert!(deleted >= 1, "the stale agent row must be swept");

    let repo = ServiceRepository::new(ctx.db_pool())?;
    assert!(
        repo.find_service_by_name(&stale_agent).await?.is_none(),
        "stale agent row is gone"
    );
    assert!(
        repo.find_service_by_name(&live_mcp).await?.is_some(),
        "non-stale (unknown-status) mcp row is retained"
    );
    Ok(())
}

async fn body_json(app: Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let resp = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .expect("response");
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.expect("body");
    let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, value)
}

#[tokio::test]
async fn handle_health_reports_healthy() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = Router::new()
        .route("/health", get(handle_health))
        .with_state((*ctx).clone());
    let (status, body) = body_json(app, "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "healthy");
    Ok(())
}

#[tokio::test]
async fn handle_health_detail_reports_checks() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = Router::new()
        .route("/health/detail", get(handle_health_detail))
        .with_state((*ctx).clone());
    let (status, body) = body_json(app, "/health/detail").await;
    assert!(status == StatusCode::OK || status == StatusCode::SERVICE_UNAVAILABLE);
    assert!(body["checks"]["database"].is_object());
    Ok(())
}
