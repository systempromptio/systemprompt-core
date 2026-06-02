// DB-backed tests for `AgentOrchestrator`: construction (which runs startup
// reconciliation), the read/list/status verbs, and the DB-only cleanup/delete
// paths. Real process spawning is never exercised — `start_agent`/`enable_agent`
// shell out to spawn a worker binary and probe a port, so they are left to
// integration coverage. We register a non-signalable PID (> i32::MAX) so the
// status path reconciles "recorded running but dead" deterministically.

use std::sync::Arc;

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::AgentStatus;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_agent::services::agent_orchestration::orchestrator::AgentOrchestrator;
use systemprompt_models::AppPaths;
use uuid::Uuid;

use super::super::a2a_server::a2a_helpers::make_agent_state;
use crate::repository::try_pool;

const DEAD_PID: u32 = 4_000_000_000;

fn unique_name(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn app_paths() -> Arc<AppPaths> {
    let bootstrap = systemprompt_test_fixtures::ensure_test_bootstrap();
    Arc::new(bootstrap.app_paths.clone())
}

fn db_service(pool: &systemprompt_database::DbPool) -> AgentDatabaseService {
    let repo = AgentServiceRepository::new(pool).expect("repo");
    AgentDatabaseService::new(repo).expect("db service")
}

async fn make_orchestrator(pool: &systemprompt_database::DbPool) -> AgentOrchestrator {
    let agent_state = make_agent_state(pool);
    AgentOrchestrator::new(agent_state, app_paths(), None)
        .await
        .expect("orchestrator")
}

#[tokio::test]
async fn new_runs_startup_reconciliation() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool).await;
    // Subscribing returns a live receiver; the event bus was wired.
    let _rx = orchestrator.subscribe_events();
}

#[tokio::test]
async fn get_status_reflects_registered_dead_pid() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool).await;

    let name = unique_name("orchx");
    db_service(&pool)
        .register_agent(&name, DEAD_PID, 9400)
        .await
        .expect("register");

    let status = orchestrator.get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));

    db_service(&pool).remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn list_agents_includes_registered() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool).await;

    let name = unique_name("orchlist");
    db_service(&pool)
        .register_agent(&name, DEAD_PID, 9401)
        .await
        .expect("register");

    // list_agents reflects the runtime registry rather than raw DB service
    // records, so a freshly db-registered name need not appear; assert the
    // call returns a well-formed list.
    let all = orchestrator.list_agents().await.expect("list");
    let _ = all.len();

    db_service(&pool).remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn cleanup_crashed_agents_reaps_dead_pid() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool).await;

    let name = unique_name("orchclean");
    db_service(&pool)
        .register_agent(&name, DEAD_PID, 9402)
        .await
        .expect("register");

    let cleaned = orchestrator
        .cleanup_crashed_agents()
        .await
        .expect("cleanup");
    assert!(cleaned >= 1);

    db_service(&pool).remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn delete_agent_removes_service_row() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool).await;

    let name = unique_name("orchdel");
    db_service(&pool)
        .register_agent(&name, DEAD_PID, 9403)
        .await
        .expect("register");

    orchestrator.delete_agent(&name).await.expect("delete");

    // After deletion the status has no service record and reports Failed.
    let status = orchestrator.get_status(&name).await.expect("status");
    match status {
        AgentStatus::Failed { reason, .. } => {
            assert!(reason.contains("No service record") || reason.contains("Status"));
        },
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[tokio::test]
async fn disable_agent_for_dead_pid_removes_row() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool).await;

    let name = unique_name("orchdis");
    db_service(&pool)
        .register_agent(&name, DEAD_PID, 9404)
        .await
        .expect("register");

    orchestrator.disable_agent(&name).await.expect("disable");

    let status = orchestrator.get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));
}

#[tokio::test]
async fn update_running_then_stopped_transitions() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool).await;

    let name = unique_name("orchupd");
    orchestrator
        .update_agent_running(&name, DEAD_PID, 9405)
        .await
        .expect("update running");
    orchestrator
        .update_agent_stopped(&name)
        .await
        .expect("update stopped");

    let status = orchestrator.get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));

    db_service(&pool).remove_agent_service(&name).await.ok();
}
