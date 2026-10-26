// DB-backed tests for `AgentLifecycle` operations that do not spawn a worker
// process: the start/restart/enable failure prologue for unregistered agents
// (config lookup fails before any spawn), disabling an agent whose process is
// already dead, and crash cleanup when no service record exists.

use std::sync::Arc;
use systemprompt_identifiers::InstanceId;

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::AgentStatus;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_agent::services::agent_orchestration::lifecycle::AgentLifecycle;
use systemprompt_config::paths::AppPaths;

use systemprompt_test_fixtures::{test_db_pool, unique_agent_name, unique_instance};

// Why: `services.pid` is an `INTEGER` column, so a dead pid must fit i32 while
// still lying far above any pid_max a kernel will hand out.
const DEAD_PID: u32 = 2_000_000_000;

fn app_paths() -> Arc<AppPaths> {
    let bootstrap = systemprompt_test_fixtures::ensure_test_bootstrap();
    Arc::new(bootstrap.app_paths.clone())
}

fn lifecycle(pool: &systemprompt_database::DbPool, instance: &InstanceId) -> AgentLifecycle {
    AgentLifecycle::new(
        AgentServiceRepository::new(pool, instance.clone()),
        app_paths(),
        systemprompt_test_fixtures::test_spawner(),
        crate::spawn_environment(),
        &systemprompt_test_fixtures::fixture_services_view(),
    )
}

fn db_service(pool: &systemprompt_database::DbPool, instance: &InstanceId) -> AgentDatabaseService {
    let repo = AgentServiceRepository::new(pool, instance.clone());
    AgentDatabaseService::new(repo, &systemprompt_test_fixtures::fixture_services_view())
}

#[tokio::test]
async fn start_agent_unknown_agent_fails_before_spawn() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool, &instance);

    let name = unique_agent_name("lc_missing");
    let result = lifecycle.start_agent(&name, None).await;
    assert!(result.is_err(), "unregistered agent must not start");
}

#[tokio::test]
async fn start_agent_unknown_agent_creates_no_service_state() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool, &instance);
    let db = db_service(&pool, &instance);
    let name = unique_agent_name("lc_unknown_event");

    let err = lifecycle
        .start_agent(&name, None)
        .await
        .expect_err("an unknown agent cannot start");
    assert!(err.to_string().contains(name.as_str()));
    assert!(!db.agent_exists(&name).expect("service lookup"));
    assert!(
        db.repository
            .find_agent_status(&name)
            .await
            .expect("row lookup")
            .is_none(),
        "config lookup fails before a process transition"
    );
}

#[tokio::test]
async fn enable_agent_delegates_to_start() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool, &instance);

    let name = unique_agent_name("lc_enable");
    let result = lifecycle.enable_agent(&name, None).await;
    assert!(result.is_err(), "unregistered agent must not enable");
}

#[tokio::test]
async fn restart_agent_with_dead_pid_row_fails_on_missing_config() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool, &instance);
    let db = db_service(&pool, &instance);

    let name = unique_agent_name("lc_restart");
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
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool, &instance);
    let db = db_service(&pool, &instance);

    let name = unique_agent_name("lc_disable");
    db.register_agent(&name, DEAD_PID, 9411)
        .await
        .expect("register");

    lifecycle.disable_agent(&name).await.expect("disable");

    let exists = db.agent_exists(&name).expect("exists");
    assert!(!exists, "disable must remove the service row");
}

#[tokio::test]
async fn cleanup_crashed_agent_without_record_is_noop() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool, &instance);

    let name = unique_agent_name("lc_cleanup");
    lifecycle
        .cleanup_crashed_agent(&name)
        .await
        .expect("cleanup is a no-op without a record");
}

#[tokio::test]
async fn cleanup_crashed_agent_transitions_dead_running_record_to_failed() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool, &instance);
    let db = db_service(&pool, &instance);
    let name = unique_agent_name("lc_crashed");

    db.register_agent(&name, DEAD_PID, 9413)
        .await
        .expect("register running record");
    let row = sqlx::query_as::<_, (String, Option<i32>, Option<i32>)>(
        "SELECT status, pid, port FROM services WHERE instance_id = $1 AND name = $2",
    )
    .bind(instance.as_str())
    .bind(name.as_str())
    .fetch_one(pool.pool().as_ref())
    .await
    .expect("raw running service row");
    assert_eq!(
        row,
        ("running".to_owned(), Some(DEAD_PID as i32), Some(9413))
    );

    lifecycle
        .cleanup_crashed_agent(&name)
        .await
        .expect("cleanup");

    assert!(matches!(
        db.get_status(&name).await.expect("failed status"),
        AgentStatus::Failed { .. }
    ));
    db.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn disable_removes_the_service_row() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool, &instance);
    let db = db_service(&pool, &instance);

    let name = unique_agent_name("lc_bus_dis");
    db.register_agent(&name, DEAD_PID, 9412)
        .await
        .expect("register");

    lifecycle.disable_agent(&name).await.expect("disable");

    assert!(
        db.repository
            .find_agent_status(&name)
            .await
            .expect("row lookup")
            .is_none()
    );
}

#[tokio::test]
async fn restart_unconfigured_agent_fails() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lifecycle = lifecycle(&pool, &instance);

    let name = unique_agent_name("lc_restart_missing");
    let result = lifecycle.restart_agent(&name, None).await;
    assert!(result.is_err(), "unconfigured agent must not restart");
}
