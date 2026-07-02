//! Coverage for the process health monitor, health checker, and health summary
//! aggregation.
//!
//! Drives [`ProcessMonitor`] against the fixture database: lifecycle
//! (start/stop/drop/double-start), on-demand
//! [`ProcessMonitor::health_check_all`] over live and dead PIDs, and the
//! background monitoring loop that marks a vanished PID's service `error`. Also
//! exercises [`HealthChecker`] retries against a `wiremock` upstream and the
//! [`HealthSummary`] / [`ModuleHealth`] arithmetic.

use std::process::Command;
use std::time::Duration;

use systemprompt_api::services::health::{
    HealthChecker, HealthSummary, ModuleHealth, ProcessMonitor,
};
use systemprompt_database::{CreateServiceInput, ServiceRepository};
use uuid::Uuid;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;

fn unique_name(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

async fn register_running(
    pool: &systemprompt_database::DbPool,
    name: &str,
    module: &str,
    pid: Option<i32>,
) -> anyhow::Result<()> {
    let repo = ServiceRepository::new(pool)?;
    repo.create_service(CreateServiceInput {
        name,
        module_name: module,
        status: "running",
        port: 0,
        binary_mtime: None,
    })
    .await?;
    if let Some(pid) = pid {
        repo.update_service_pid(name, pid).await?;
    }
    Ok(())
}

fn dead_pid() -> i32 {
    let mut child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn sleep");
    let pid = child.id() as i32;
    child.kill().expect("kill child");
    child.wait().expect("reap child");
    pid
}

#[tokio::test]
async fn monitor_lifecycle_start_stop_and_double_start() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let mut monitor = ProcessMonitor::new(pool);
    assert!(!monitor.is_running());

    monitor.start();
    assert!(monitor.is_running());

    monitor.start();
    assert!(monitor.is_running());

    monitor.stop();
    assert!(!monitor.is_running());

    monitor.stop();
    assert!(!monitor.is_running());
    Ok(())
}

#[tokio::test]
async fn monitor_drop_aborts_running_loop() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let mut monitor = ProcessMonitor::with_interval(pool, Duration::from_secs(60));
    monitor.start();
    assert!(monitor.is_running());
    drop(monitor);
    Ok(())
}

#[tokio::test]
async fn health_check_all_counts_live_pid_as_healthy() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let name = unique_name("hc-live");
    let own_pid = std::process::id() as i32;
    register_running(&pool, &name, "custom", Some(own_pid)).await?;

    let monitor = ProcessMonitor::new(pool);
    let summary = monitor.health_check_all().await?;

    assert!(summary.total_healthy() >= 1, "own PID should be healthy");
    assert!(
        summary.modules.contains_key("custom"),
        "custom module present in summary"
    );
    Ok(())
}

#[tokio::test]
async fn health_check_all_counts_dead_pid_as_crashed() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let module = unique_name("mod-dead");
    let name = unique_name("hc-dead");
    register_running(&pool, &name, &module, Some(dead_pid())).await?;

    let monitor = ProcessMonitor::new(pool);
    let summary = monitor.health_check_all().await?;

    let health = summary.modules.get(&module).copied().unwrap_or_default();
    assert_eq!(health.crashed, 1, "dead PID marked crashed for its module");
    assert_eq!(health.healthy, 0);
    Ok(())
}

#[tokio::test]
async fn monitor_loop_marks_vanished_service_as_error() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let name = unique_name("loop-dead");
    register_running(&pool, &name, "custom", Some(dead_pid())).await?;

    let mut monitor = ProcessMonitor::with_interval(pool.clone(), Duration::from_millis(50));
    monitor.start();

    let repo = ServiceRepository::new(&pool)?;
    let mut status = String::new();
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Some(svc) = repo.find_service_by_name(&name).await? {
            status = svc.status;
            if status == "error" {
                break;
            }
        }
    }
    monitor.stop();

    assert_eq!(
        status, "error",
        "monitor loop should mark vanished PID error"
    );
    Ok(())
}

#[tokio::test]
async fn health_checker_succeeds_on_200() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let checker = HealthChecker::new(format!("{}/health", server.uri()))
        .with_max_retries(2)
        .with_retry_delay(Duration::from_millis(10));
    checker.check().await?;
    Ok(())
}

#[tokio::test]
async fn health_checker_fails_after_retries_on_500() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let checker = HealthChecker::new(format!("{}/health", server.uri()))
        .with_max_retries(2)
        .with_retry_delay(Duration::from_millis(10));
    assert!(checker.check().await.is_err(), "500 upstream must fail");
    Ok(())
}

#[test]
fn health_summary_arithmetic_and_flags() {
    let mut summary = HealthSummary::default();
    assert!(summary.is_all_healthy());
    assert_eq!(summary.total_healthy(), 0);
    assert_eq!(summary.total_crashed(), 0);

    summary.modules.insert(
        "a".to_owned(),
        ModuleHealth {
            healthy: 2,
            crashed: 0,
        },
    );
    let mut b = ModuleHealth {
        healthy: 1,
        crashed: 0,
    };
    b += ModuleHealth {
        healthy: 0,
        crashed: 3,
    };
    summary.modules.insert("b".to_owned(), b);

    assert_eq!(summary.total_healthy(), 3);
    assert_eq!(summary.total_crashed(), 3);
    assert!(!summary.is_all_healthy());
}
