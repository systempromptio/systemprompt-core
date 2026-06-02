// DB-backed tests for AgentDatabaseService: status reconciliation, lifecycle
// transitions, listing, and orphan cleanup. Each test early-returns when no
// test database is configured (mirrors the repository test guard).
//
// PIDs above i32::MAX are non-signalable, so `process::process_exists` returns
// false for them without ever touching a real process. We use such a PID to
// drive the "recorded running but process dead" reconciliation branch
// deterministically.

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::AgentStatus;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_test_fixtures::ensure_test_bootstrap;
use uuid::Uuid;

use crate::repository::try_pool;

// A PID that can never name a live, signalable process (> i32::MAX).
const DEAD_PID: u32 = 4_000_000_000;

fn unique_name(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

async fn service(pool: &systemprompt_database::DbPool) -> AgentDatabaseService {
    ensure_test_bootstrap();
    let _skills = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repo = AgentServiceRepository::new(pool).expect("repo");
    AgentDatabaseService::new(repo).expect("db service")
}

#[tokio::test]
async fn register_then_status_reconciles_dead_pid_to_failed() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    let name = unique_name("orch-dead");

    svc.register_agent(&name, DEAD_PID, 9300)
        .await
        .expect("register");

    // Stored status is 'running' but the PID is not a live process, so
    // get_status marks it failed and reports Failed.
    let status = svc.get_status(&name).await.expect("status");
    match status {
        AgentStatus::Failed { reason, .. } => {
            assert!(reason.contains("died") || reason.contains("Status"));
        },
        other => panic!("expected Failed, got {other:?}"),
    }

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn status_no_record_is_failed() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    let status = svc
        .get_status(&unique_name("orch-missing"))
        .await
        .expect("status");
    match status {
        AgentStatus::Failed { reason, .. } => assert!(reason.contains("No service record")),
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[tokio::test]
async fn status_starting_is_failed_with_starting_reason() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    let name = unique_name("orch-starting");
    svc.register_agent_starting(&name, DEAD_PID, 9301)
        .await
        .expect("register starting");

    let status = svc.get_status(&name).await.expect("status");
    match status {
        AgentStatus::Failed { reason, .. } => assert!(reason.contains("starting")),
        other => panic!("expected Failed, got {other:?}"),
    }

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn status_stopped_is_failed() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    let name = unique_name("orch-stopped");
    svc.register_agent(&name, DEAD_PID, 9302)
        .await
        .expect("register");
    svc.update_agent_stopped(&name).await.expect("stop");

    let status = svc.get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn mark_failed_and_error_message() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    let name = unique_name("orch-markfail");
    svc.register_agent(&name, DEAD_PID, 9303)
        .await
        .expect("register");

    svc.mark_failed(&name).await.expect("mark failed");
    let msg = svc.get_error_message(&name).await.expect("err msg");
    assert!(msg.starts_with("Status:"));

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn error_message_no_record() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    let msg = svc
        .get_error_message(&unique_name("orch-noerr"))
        .await
        .expect("err msg");
    assert_eq!(msg, "No service record");
}

#[tokio::test]
async fn list_running_agents_includes_registered() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    let name = unique_name("orch-listrun");
    svc.register_agent(&name, DEAD_PID, 9304)
        .await
        .expect("register");

    let running = svc.list_running_agents().await.expect("list");
    assert!(running.iter().any(|n| n == &name));

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn cleanup_orphaned_services_marks_dead_pids() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    let name = unique_name("orch-orphan");
    svc.register_agent(&name, DEAD_PID, 9305)
        .await
        .expect("register");

    let cleaned = svc.cleanup_orphaned_services().await.expect("cleanup");
    // At least our orphan should be reaped.
    assert!(cleaned >= 1);

    let status = svc.get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn lifecycle_register_starting_mark_running_then_stopped() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    let name = unique_name("orch-lifecycle");

    svc.register_agent_starting(&name, DEAD_PID, 9306)
        .await
        .expect("starting");
    svc.mark_running(&name).await.expect("running");
    // update_agent_running upserts the row back to running.
    svc.update_agent_running(&name, DEAD_PID, 9307)
        .await
        .expect("update running");
    svc.update_health_status(&name, "degraded")
        .await
        .expect("health");
    svc.update_agent_stopped(&name).await.expect("stopped");

    let status = svc.get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn mark_error_and_mark_crashed() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    let name = unique_name("orch-crash");
    svc.register_agent(&name, DEAD_PID, 9308)
        .await
        .expect("register");
    svc.mark_error(&name).await.expect("mark error");
    svc.mark_crashed(&name).await.expect("mark crashed");

    let status = svc.get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn agent_exists_false_for_unconfigured() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    let exists = svc
        .agent_exists("__no_such_configured_agent")
        .await
        .expect("exists");
    assert!(!exists);
}

#[tokio::test]
async fn get_agent_config_unknown_errors() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    let err = svc
        .get_agent_config("__no_such_agent_cfg")
        .await
        .expect_err("not found");
    assert!(format!("{err}").contains("not found"));
}

#[tokio::test]
async fn list_all_agents_empty_default_config() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    // Default test config has no agents configured.
    let all = svc.list_all_agents().await.expect("list all");
    assert!(all.is_empty());
}

#[tokio::test]
async fn remove_unknown_service_is_ok() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = service(&pool).await;
    svc.remove_agent_service(&unique_name("orch-ghost"))
        .await
        .expect("remove ok");
}
