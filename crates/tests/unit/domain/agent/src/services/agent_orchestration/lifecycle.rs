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

#[tokio::test]
async fn disable_with_event_bus_publishes_agent_disabled() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let bus = Arc::new(
        systemprompt_agent::services::agent_orchestration::event_bus::AgentEventBus::new(16),
    );
    let mut events = bus.subscribe();
    let lifecycle = lifecycle(&pool).with_event_bus(Arc::clone(&bus));
    let db = db_service(&pool);

    let name = unique_name("lc_bus_dis");
    db.register_agent(&name, DEAD_PID, 9412)
        .await
        .expect("register");

    lifecycle.disable_agent(&name).await.expect("disable");

    let event = events.try_recv().expect("disable must publish an event");
    match event {
        systemprompt_agent::services::agent_orchestration::events::AgentEvent::AgentDisabled {
            agent_id,
        } => assert_eq!(agent_id.as_str(), name),
        other => panic!("expected AgentDisabled, got {other:?}"),
    }
}

#[tokio::test]
async fn restart_with_event_bus_publishes_restart_requested_before_failing() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let bus = Arc::new(
        systemprompt_agent::services::agent_orchestration::event_bus::AgentEventBus::new(16),
    );
    let mut events = bus.subscribe();
    let lifecycle = lifecycle(&pool).with_event_bus(bus);

    let name = unique_name("lc_bus_restart");
    let result = lifecycle.restart_agent(&name, None).await;
    assert!(result.is_err(), "unconfigured agent must not restart");

    let event = events.try_recv().expect("restart must publish an event");
    match event {
        systemprompt_agent::services::agent_orchestration::events::AgentEvent::AgentRestartRequested {
            agent_id,
            reason,
        } => {
            assert_eq!(agent_id.as_str(), name);
            assert!(reason.to_lowercase().contains("restart"));
        },
        other => panic!("expected AgentRestartRequested, got {other:?}"),
    }
}

#[tokio::test]
async fn free_function_verbs_cover_missing_agent_paths() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    use systemprompt_agent::services::agent_orchestration::lifecycle as verbs;

    let name = unique_name("lc_free");
    assert!(
        verbs::start_agent(&pool, app_paths(), &name, None)
            .await
            .is_err()
    );
    assert!(
        verbs::enable_agent(&pool, app_paths(), &name, None)
            .await
            .is_err()
    );
    assert!(
        verbs::restart_agent(&pool, app_paths(), &name, None)
            .await
            .is_err()
    );
    verbs::disable_agent(&pool, app_paths(), &name)
        .await
        .expect("disable of an unregistered agent is a no-op removal");
}
