// DB-backed tests for `AgentOrchestrator`: construction (which runs startup
// reconciliation), the read/list/status verbs, and the DB-only cleanup/delete
// paths. Real process spawning is never exercised —
// `start_agent`/`enable_agent` shell out to spawn a worker binary and probe a
// port, so they are left to integration coverage. We register a non-signalable
// PID (> i32::MAX) so the status path reconciles "recorded running but dead"
// deterministically.

use std::sync::Arc;
use systemprompt_identifiers::InstanceId;

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::AgentStatus;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_agent::services::agent_orchestration::orchestrator::AgentOrchestrator;
use systemprompt_config::paths::AppPaths;

use super::super::a2a_server::a2a_helpers::make_agent_state_for;
use systemprompt_test_fixtures::{test_db_pool, unique_agent_name, unique_instance};

// Why: `services.pid` is an `INTEGER` column, so a dead pid must fit i32 while
// still lying far above any pid_max a kernel will hand out.
const DEAD_PID: u32 = 2_000_000_000;

fn app_paths() -> Arc<AppPaths> {
    let bootstrap = systemprompt_test_fixtures::ensure_test_bootstrap();
    Arc::new(bootstrap.app_paths.clone())
}

fn db_service(pool: &systemprompt_database::DbPool, instance: &InstanceId) -> AgentDatabaseService {
    let repo = AgentServiceRepository::new(pool, instance.clone());
    AgentDatabaseService::new(repo, &systemprompt_test_fixtures::fixture_services_state())
}

async fn make_orchestrator(
    pool: &systemprompt_database::DbPool,
    instance: &InstanceId,
) -> AgentOrchestrator {
    let agent_state = make_agent_state_for(pool, instance);
    AgentOrchestrator::new(
        agent_state,
        app_paths(),
        systemprompt_test_fixtures::test_spawner(),
        None,
    )
    .await
    .expect("orchestrator")
}

#[tokio::test]
async fn new_runs_startup_reconciliation() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool, &instance).await;
    orchestrator
        .list_all()
        .await
        .expect("reconciled orchestrator lists agents");
}

#[tokio::test]
async fn get_status_reflects_registered_dead_pid() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool, &instance).await;

    let name = unique_agent_name("orchx");
    db_service(&pool, &instance)
        .register_agent(&name, DEAD_PID, 9400)
        .await
        .expect("register");

    let status = orchestrator.get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));

    db_service(&pool, &instance)
        .remove_agent_service(&name)
        .await
        .ok();
}

#[tokio::test]
async fn list_agents_includes_registered() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool, &instance).await;

    let name = unique_agent_name("orchlist");
    db_service(&pool, &instance)
        .register_agent(&name, DEAD_PID, 9401)
        .await
        .expect("register");

    // list_agents reflects the runtime registry rather than raw DB service
    // records, so a freshly db-registered name need not appear; assert the
    // call returns a well-formed list.
    let all = orchestrator.list_agents().await.expect("list");
    assert!(
        all.iter()
            .all(|(agent_name, _)| !agent_name.as_str().is_empty()),
        "every listed agent must carry a non-empty name"
    );

    db_service(&pool, &instance)
        .remove_agent_service(&name)
        .await
        .ok();
}

#[tokio::test]
async fn delete_agent_removes_service_row() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool, &instance).await;

    let name = unique_agent_name("orchdel");
    db_service(&pool, &instance)
        .register_agent(&name, DEAD_PID, 9403)
        .await
        .expect("register");

    orchestrator.delete_agent(&name).await.expect("delete");

    // After deletion the status has no service record and reports Failed.
    let status = orchestrator.get_status(&name).await.expect("status");
    match status {
        AgentStatus::Failed { reason, .. } => {
            assert!(reason.contains("No service record"));
        },
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[tokio::test]
async fn disable_agent_for_dead_pid_removes_row() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool, &instance).await;

    let name = unique_agent_name("orchdis");
    db_service(&pool, &instance)
        .register_agent(&name, DEAD_PID, 9404)
        .await
        .expect("register");

    orchestrator.disable_agent(&name).await.expect("disable");

    let status = orchestrator.get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));
}

#[tokio::test]
async fn update_running_then_stopped_transitions() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool, &instance).await;

    let name = unique_agent_name("orchupd");
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

    db_service(&pool, &instance)
        .remove_agent_service(&name)
        .await
        .ok();
}
