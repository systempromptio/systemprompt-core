// DB-backed tests for `AgentLifecycle` operations that do not spawn a worker
// process: the start/restart/enable failure prologue for unregistered agents
// (config lookup fails before any spawn), disabling an agent whose process is
// already dead, and crash cleanup when no service record exists.

use std::sync::Arc;

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_agent::services::agent_orchestration::lifecycle::AgentLifecycle;
use systemprompt_models::AppPaths;
use uuid::Uuid;

use crate::repository::try_pool;

const DEAD_PID: u32 = 4_000_000_000;

fn unique_name(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn app_paths() -> Arc<AppPaths> {
    let bootstrap = systemprompt_test_fixtures::ensure_test_bootstrap();
    Arc::new(bootstrap.app_paths.clone())
}

fn lifecycle(pool: &systemprompt_database::DbPool) -> AgentLifecycle {
    AgentLifecycle::new(pool, app_paths()).expect("lifecycle")
}

fn db_service(pool: &systemprompt_database::DbPool) -> AgentDatabaseService {
    let repo = AgentServiceRepository::new(pool).expect("repo");
    AgentDatabaseService::new(repo).expect("db service")
}

#[tokio::test]
async fn start_agent_unknown_agent_fails_before_spawn() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool);

    let name = unique_name("lc_missing");
    let result = lifecycle.start_agent(&name, None).await;
    assert!(result.is_err(), "unregistered agent must not start");
}

#[tokio::test]
async fn enable_agent_delegates_to_start() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool);

    let name = unique_name("lc_enable");
    let result = lifecycle.enable_agent(&name, None).await;
    assert!(result.is_err(), "unregistered agent must not enable");
}

#[tokio::test]
async fn restart_agent_with_dead_pid_row_fails_on_missing_config() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool);
    let db = db_service(&pool);

    let name = unique_name("lc_restart");
    db.register_agent(&name, DEAD_PID, 9410)
        .await
        .expect("register");

    let result = lifecycle.restart_agent(&name, None).await;
    assert!(
        result.is_err(),
        "restart of an unconfigured agent must fail"
    );

    db.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn disable_agent_with_dead_pid_removes_service_row() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool);
    let db = db_service(&pool);

    let name = unique_name("lc_disable");
    db.register_agent(&name, DEAD_PID, 9411)
        .await
        .expect("register");

    lifecycle.disable_agent(&name).await.expect("disable");

    let exists = db.agent_exists(&name).await.expect("exists");
    assert!(!exists, "disable must remove the service row");
}

#[tokio::test]
async fn cleanup_crashed_agent_without_record_is_noop() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool);

    let name = unique_name("lc_cleanup");
    lifecycle
        .cleanup_crashed_agent(&name)
        .await
        .expect("cleanup is a no-op without a record");
}
