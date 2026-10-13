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
use std::time::Duration;
use systemprompt_api::services::server::health::human_bytes;
use systemprompt_api::services::server::health_detail::handle_health_detail;
use systemprompt_api::services::server::lifecycle::reconciliation::cleanup_stale_service_entries;
use systemprompt_api::services::server::{handle_health, readiness, scheduler_health, shutdown};
use systemprompt_database::{CreateServiceInput, ServiceRepository};
use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::services::{ServiceModule, ServiceStatus};
use systemprompt_runtime::{AppContext, ShutdownRequest};
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
    status: ServiceStatus,
    pid: Option<i32>,
) -> anyhow::Result<()> {
    let repo = ServiceRepository::new(ctx.db_pool(), ctx.config().instance_id.clone());
    let name = ServiceName::new(name);
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: ServiceModule::Mcp,
        status,
        port: 0,
        binary_mtime: None,
    })
    .await?;
    if let Some(pid) = pid {
        repo.update_service_pid(&name, pid).await?;
    }
    Ok(())
}

async fn seed_agent_service(
    ctx: &AppContext,
    name: &str,
    status: ServiceStatus,
    pid: Option<i32>,
) -> anyhow::Result<()> {
    let repo = ServiceRepository::new(ctx.db_pool(), ctx.config().instance_id.clone());
    let name = ServiceName::new(name);
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: ServiceModule::Agent,
        status,
        port: 0,
        binary_mtime: None,
    })
    .await?;
    if let Some(pid) = pid {
        repo.update_service_pid(&name, pid).await?;
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

#[tokio::test]
async fn cleanup_removes_stale_mcp_rows() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let name = format!("stale-mcp-{}", Uuid::new_v4().simple());
    seed_mcp_service(&ctx, &name, ServiceStatus::Error, None).await?;

    let deleted = cleanup_stale_service_entries(&ctx, None).await?;
    assert!(deleted >= 1, "the error-status row must be swept");

    let repo = ServiceRepository::new(ctx.db_pool(), ctx.config().instance_id.clone());
    assert!(
        repo.find_service_by_name(&ServiceName::new(name.as_str()))
            .await?
            .is_none(),
        "stale row is gone"
    );
    Ok(())
}

#[tokio::test]
async fn shutdown_drain_clears_dead_and_recycled_children() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let dead = format!("dead-mcp-{}", Uuid::new_v4().simple());
    let recycled = format!("recycled-mcp-{}", Uuid::new_v4().simple());
    seed_mcp_service(&ctx, &dead, ServiceStatus::Running, Some(dead_pid())).await?;
    let mut foreign = std::process::Command::new("sleep").arg("30").spawn()?;
    seed_mcp_service(
        &ctx,
        &recycled,
        ServiceStatus::Running,
        Some(i32::try_from(foreign.id())?),
    )
    .await?;

    shutdown::terminate_children(&ctx).await;
    let foreign_survived = foreign.try_wait()?.is_none();
    foreign.kill()?;
    foreign.wait()?;
    assert!(foreign_survived, "an unmarked live pid is never signalled");

    let repo = ServiceRepository::new(ctx.db_pool(), ctx.config().instance_id.clone());
    let recycled_row = repo
        .find_service_by_name(&ServiceName::new(recycled.as_str()))
        .await?
        .expect("recycled row still present");
    assert_ne!(
        recycled_row.status,
        ServiceStatus::Running,
        "a live non-child pid is cleared, not signalled"
    );

    shutdown::drain(&ctx, None).await;
    Ok(())
}

/// Both halves of the drain guard, in one test because the readiness broadcast
/// is process-wide: as separate tests, the shutdown announced by one could arm
/// the other's grace window and make the pair flaky.
#[tokio::test(start_paused = true)]
async fn drain_grace_bounds_the_drain_and_not_the_server() {
    let grace = Duration::from_millis(shutdown::AXUM_DRAIN_GRACE_MS);

    // Never signalled: the guard must not be a deadline on healthy serving.
    let unsignalled = tokio::time::timeout(
        grace * 3,
        shutdown::join_within_drain_grace(
            std::future::pending::<anyhow::Result<()>>(),
            &ShutdownRequest::default(),
        ),
    )
    .await;
    assert!(
        unsignalled.is_err(),
        "an unsignalled server must keep serving, not tear itself down"
    );

    // Signalled, then wedged: the drain is abandoned rather than the process.
    // The signal is spawned rather than sent inline because the guard
    // subscribes on first poll, and a broadcast delivers nothing sent earlier.
    let wedged = tokio::spawn(async {
        shutdown::join_within_drain_grace(
            std::future::pending::<anyhow::Result<()>>(),
            &ShutdownRequest::default(),
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    readiness::signal_shutdown();

    let result = tokio::time::timeout(grace * 3, wedged)
        .await
        .expect("a wedged drain must be abandoned once shutdown is signalled")
        .expect("join task panicked");
    assert!(
        result.is_ok(),
        "abandoning a wedged drain must not fail the run loop"
    );
}

#[tokio::test(start_paused = true)]
async fn drain_grace_leaves_room_for_child_termination() {
    assert!(
        shutdown::AXUM_DRAIN_GRACE_MS > shutdown::CHILD_SHUTDOWN_GRACE_MS,
        "a drain that consumes the whole budget would strand every child"
    );

    let served =
        shutdown::join_within_drain_grace(async { Ok(()) }, &ShutdownRequest::default()).await;
    assert!(served.is_ok(), "a clean drain returns the serve result");
}

#[tokio::test]
async fn cleanup_sweeps_stale_agent_row_and_keeps_non_stale_mcp() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let stale_agent = format!("stale-agent-{}", Uuid::new_v4().simple());
    let live_mcp = format!("live-mcp-{}", Uuid::new_v4().simple());
    seed_agent_service(&ctx, &stale_agent, ServiceStatus::Error, None).await?;
    seed_mcp_service(&ctx, &live_mcp, ServiceStatus::Starting, None).await?;

    let deleted = cleanup_stale_service_entries(&ctx, None).await?;
    assert!(deleted >= 1, "the stale agent row must be swept");

    let repo = ServiceRepository::new(ctx.db_pool(), ctx.config().instance_id.clone());
    assert!(
        repo.find_service_by_name(&ServiceName::new(stale_agent.as_str()))
            .await?
            .is_none(),
        "stale agent row is gone"
    );
    assert!(
        repo.find_service_by_name(&ServiceName::new(live_mcp.as_str()))
            .await?
            .is_some(),
        "non-stale (starting) mcp row is retained"
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
async fn handle_health_is_degraded_until_the_event_relay_listens() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = Router::new()
        .route("/health", get(handle_health))
        .with_state((*ctx).clone());
    let (status, body) = body_json(app, "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "degraded");
    assert_eq!(body["events"]["relay"], "not_started");
    Ok(())
}

#[tokio::test]
async fn handle_health_reports_healthy_with_a_listening_relay() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let instance_id = ctx.config().instance_id.clone();
    let handle = systemprompt_events::PostgresEventBridge::new(
        ctx.db_pool().write_pool().as_ref().clone(),
        instance_id,
    )
    .start();
    let listening = tokio::time::timeout(Duration::from_secs(10), handle.listening())
        .await
        .expect("the relay starts listening within 10s");
    assert!(listening, "the relay stopped before it was listening");
    ctx.event_bridge()
        .set(handle)
        .expect("a fresh fixture context has no relay");
    let app = Router::new()
        .route("/health", get(handle_health))
        .with_state((*ctx).clone());
    let (status, body) = body_json(app, "/health").await;
    if let Some(handle) = ctx.event_bridge().get() {
        handle.shutdown().await;
    }
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
