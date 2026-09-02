// DB-backed tests for the AgentOrchestrator verbs that report over the startup
// event channel, plus the bulk-disable and comprehensive-status paths.
// `list_all_agents` is registry-driven, so the registry injected via
// `set_registry` decides what reconcile counts and what detailed status walks.
// PIDs above i32::MAX are never live, which pins every status to Failed without
// spawning a worker.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::AgentStatus;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_agent::services::agent_orchestration::orchestrator::AgentOrchestrator;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_models::{AppPaths, ServicesConfig};
use systemprompt_traits::{Phase, StartupEvent, startup_channel};
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
    let repo = AgentServiceRepository::new(
        pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    )
    .expect("repo");
    AgentDatabaseService::new(repo).expect("db service")
}

fn registry_from(entries: &[(&str, &str, u16)]) -> AgentRegistry {
    let mut agents = HashMap::new();
    for (key, name, port) in entries {
        let mut config = agent_config(name);
        config.port = *port;
        agents.insert((*key).to_owned(), config);
    }
    AgentRegistry::from_config(ServicesConfig {
        agents,
        ..ServicesConfig::default()
    })
}

async fn make_orchestrator(
    pool: &systemprompt_database::DbPool,
    entries: &[(&str, &str, u16)],
) -> AgentOrchestrator {
    let agent_state = make_agent_state(pool);
    let mut orchestrator = AgentOrchestrator::new(agent_state, app_paths(), None)
        .await
        .expect("orchestrator");
    orchestrator.set_registry(registry_from(entries));
    orchestrator
}

#[tokio::test]
async fn reconcile_reports_the_agent_phase_and_the_registry_totals() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let a = unique_name("recon_a");
    let b = unique_name("recon_b");
    let orchestrator = make_orchestrator(&pool, &[(&a, &a, 39470), (&b, &b, 39471)]).await;

    let db = db_service(&pool);
    db.register_agent(&a, DEAD_PID, 39470).await.expect("reg a");
    db.register_agent(&b, DEAD_PID, 39471).await.expect("reg b");

    let (tx, mut rx) = startup_channel();
    let result = orchestrator.reconcile(Some(&tx)).await;
    drop(tx);

    db.remove_agent_service(&a).await.ok();
    db.remove_agent_service(&b).await.ok();
    result.expect("reconcile");

    let mut started = false;
    let mut completed = false;
    let mut totals = None;
    while let Ok(event) = rx.try_recv() {
        match event {
            StartupEvent::PhaseStarted {
                phase: Phase::Agents,
            } => started = true,
            StartupEvent::PhaseCompleted {
                phase: Phase::Agents,
            } => completed = true,
            StartupEvent::AgentReconciliationComplete { running, total } => {
                totals = Some((running, total));
            },
            _ => {},
        }
    }

    assert!(started, "reconcile opens the Agents phase");
    assert!(completed, "reconcile closes the Agents phase");
    assert_eq!(
        totals,
        Some((0, 2)),
        "both registry agents are counted, neither is live"
    );
}

#[tokio::test]
async fn detailed_status_falls_back_when_the_registry_key_differs_from_the_name() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let key = unique_name("recon_key");
    let declared = unique_name("recon_declared");
    let orchestrator = make_orchestrator(&pool, &[(&key, &declared, 39472)]).await;

    let info = orchestrator.get_detailed_status().await.expect("status");
    let entry = info
        .iter()
        .find(|i| i.id.as_str() == declared)
        .expect("status is keyed by the declared agent name");

    assert_eq!(
        entry.name, "Unknown",
        "a name that is not a registry key resolves no config"
    );
    assert_eq!(entry.port, 8000, "the failed-status fallback port is used");
    assert!(matches!(entry.status, AgentStatus::Failed { .. }));
}

#[tokio::test]
async fn disable_all_leaves_every_registry_agent_failed() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let a = unique_name("recon_dis_a");
    let b = unique_name("recon_dis_b");
    let orchestrator = make_orchestrator(&pool, &[(&a, &a, 39473), (&b, &b, 39474)]).await;

    let db = db_service(&pool);
    db.register_agent(&a, DEAD_PID, 39473).await.expect("reg a");
    db.register_agent(&b, DEAD_PID, 39474).await.expect("reg b");

    orchestrator.disable_all().await.expect("disable all");

    let statuses = orchestrator.list_all().await.expect("list");
    assert_eq!(statuses.len(), 2);
    assert!(
        statuses
            .iter()
            .all(|(_, status)| matches!(status, AgentStatus::Failed { .. })),
        "no agent survives a bulk disable: {statuses:?}"
    );

    db.remove_agent_service(&a).await.ok();
    db.remove_agent_service(&b).await.ok();
}

#[tokio::test]
async fn health_check_reports_a_dead_pid_as_not_running() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let name = unique_name("recon_health");
    let orchestrator = make_orchestrator(&pool, &[(&name, &name, 39475)]).await;

    let db = db_service(&pool);
    db.register_agent(&name, DEAD_PID, 39475)
        .await
        .expect("register");

    let result = orchestrator
        .health_check(&name)
        .await
        .expect("health check");
    assert!(
        !result.healthy,
        "an agent with a dead pid never reports healthy"
    );
    assert!(
        result.message.contains("not in running state"),
        "the dead pid is reconciled to a failed status before the TCP probe: {}",
        result.message
    );
    assert_eq!(result.response_time_ms, 0, "no probe was attempted");

    db.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn enable_agent_for_an_unregistered_name_is_rejected() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let orchestrator = make_orchestrator(&pool, &[]).await;

    let err = orchestrator
        .enable_agent("__no_such_agent", None)
        .await
        .expect_err("an agent absent from the registry cannot be enabled");
    assert!(
        err.to_string().contains("__no_such_agent"),
        "the error names the agent: {err}"
    );
}
