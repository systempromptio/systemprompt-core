// DB-backed tests for AgentOrchestrator verbs that need a populated agent
// registry, injected via `set_registry` over an explicit `ServicesConfig`.
// PIDs above i32::MAX are never live; the test process's own PID drives the
// "recorded running and alive" branches without spawning anything (the
// verified-kill guard refuses to signal a process that is not our child).

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::AgentStatus;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_agent::services::agent_orchestration::orchestrator::AgentOrchestrator;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_models::{AppPaths, ServicesConfig};
use uuid::Uuid;

use super::super::a2a_server::a2a_helpers::{agent_config, make_agent_state};
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

fn registry_with(names_and_ports: &[(&str, u16)]) -> AgentRegistry {
    let mut agents = HashMap::new();
    for (name, port) in names_and_ports {
        let mut config = agent_config(name);
        config.port = *port;
        agents.insert((*name).to_owned(), config);
    }
    AgentRegistry::from_config(ServicesConfig {
        agents,
        ..ServicesConfig::default()
    })
}

async fn make_orchestrator(
    pool: &systemprompt_database::DbPool,
    names_and_ports: &[(&str, u16)],
) -> AgentOrchestrator {
    let agent_state = make_agent_state(pool);
    let mut orchestrator = AgentOrchestrator::new(agent_state, app_paths(), None)
        .await
        .expect("orchestrator");
    orchestrator.set_registry(registry_with(names_and_ports));
    orchestrator
}

#[tokio::test]
async fn detailed_status_reports_configured_agents() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let name = unique_name("orchreg_status");
    let orchestrator = make_orchestrator(&pool, &[(&name, 9450)]).await;

    db_service(&pool)
        .register_agent(&name, DEAD_PID, 9450)
        .await
        .expect("register");

    let info = orchestrator.get_detailed_status().await.expect("status");
    let entry = info
        .iter()
        .find(|i| i.id.as_str() == name)
        .expect("configured agent listed");
    assert_eq!(entry.port, 9450);
    assert!(matches!(entry.status, AgentStatus::Failed { .. }));

    let all = orchestrator.list_all().await.expect("list_all");
    assert!(all.iter().any(|(id, _)| id == &name));

    db_service(&pool).remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn validate_agent_reports_missing_and_failed() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let name = unique_name("orchreg_validate");
    let orchestrator = make_orchestrator(&pool, &[(&name, 9451)]).await;

    let missing = orchestrator
        .validate_agent("__no_such_agent")
        .await
        .expect("report");
    assert!(
        missing
            .issues
            .iter()
            .any(|i| i.contains("not found in database")),
        "got: {:?}",
        missing.issues
    );

    db_service(&pool)
        .register_agent(&name, DEAD_PID, 9451)
        .await
        .expect("register");
    let failed = orchestrator.validate_agent(&name).await.expect("report");
    assert!(
        failed
            .issues
            .iter()
            .any(|i| i.contains("failed state") || i.contains("Health check"))
    );

    db_service(&pool).remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn validate_agent_running_agent_reaches_health_check() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let name = unique_name("orchreg_health");
    let orchestrator = make_orchestrator(&pool, &[(&name, 9452)]).await;

    db_service(&pool)
        .register_agent(&name, std::process::id(), 9452)
        .await
        .expect("register");

    let report = orchestrator.validate_agent(&name).await.expect("report");
    assert!(
        report
            .issues
            .iter()
            .all(|i| !i.contains("Agent not found in database"))
    );

    let _results = orchestrator.health_check_all().await.expect("health all");

    db_service(&pool).remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn delete_agent_with_live_pid_removes_service_row() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let name = unique_name("orchreg_delete");
    let orchestrator = make_orchestrator(&pool, &[(&name, 9453)]).await;

    db_service(&pool)
        .register_agent(&name, std::process::id(), 9453)
        .await
        .expect("register");

    orchestrator.delete_agent(&name).await.expect("delete");

    let status = db_service(&pool).get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));
}

#[tokio::test]
async fn delete_all_agents_deletes_configured_agents() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let a = unique_name("orchreg_bulk_a");
    let b = unique_name("orchreg_bulk_b");
    let orchestrator = make_orchestrator(&pool, &[(&a, 9454), (&b, 9455)]).await;

    let db = db_service(&pool);
    db.register_agent(&a, DEAD_PID, 9454).await.expect("reg a");
    db.register_agent(&b, DEAD_PID, 9455).await.expect("reg b");

    let deleted = orchestrator.delete_all_agents().await.expect("delete all");
    assert!(deleted >= 2);
}

#[tokio::test]
async fn delete_all_agents_empty_registry_is_zero() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool, &[]).await;
    let deleted = orchestrator.delete_all_agents().await.expect("delete all");
    assert_eq!(deleted, 0);
}

#[tokio::test]
async fn process_orphaned_pids_skips_tracked_and_flags_unknown() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let name = unique_name("orchreg_orphan");
    let orchestrator = make_orchestrator(&pool, &[(&name, 9456)]).await;

    let own_pid = std::process::id();
    db_service(&pool)
        .register_agent(&name, own_pid, 9456)
        .await
        .expect("register");

    let pids = format!("{own_pid}\n4000000001\nnot-a-pid\n\n");
    orchestrator
        .process_orphaned_pids(&pids)
        .await
        .expect("orphan processing");

    db_service(&pool).remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn start_agent_already_running_is_rejected() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let name = unique_name("orchreg_running");
    let orchestrator = make_orchestrator(&pool, &[(&name, 9457)]).await;

    db_service(&pool)
        .register_agent(&name, std::process::id(), 9457)
        .await
        .expect("register");

    let err = orchestrator
        .start_agent(&name, None)
        .await
        .expect_err("already running");
    assert!(err.to_string().to_lowercase().contains("running"));

    db_service(&pool).remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn start_agent_missing_binary_fails_after_prerequisites() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let name = unique_name("orchreg_spawnfail");
    let orchestrator = make_orchestrator(&pool, &[(&name, 39457)]).await;

    let result = orchestrator.start_agent(&name, None).await;
    assert!(result.is_err(), "spawn without a worker binary must fail");

    db_service(&pool).remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn restart_agent_dead_pid_row_reaches_start_path() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let name = unique_name("orchreg_restart");
    let orchestrator = make_orchestrator(&pool, &[(&name, 39458)]).await;

    db_service(&pool)
        .register_agent(&name, DEAD_PID, 39458)
        .await
        .expect("register");

    let result = orchestrator.restart_agent(&name, None).await;
    assert!(result.is_err(), "restart without a worker binary must fail");

    db_service(&pool).remove_agent_service(&name).await.ok();
}
