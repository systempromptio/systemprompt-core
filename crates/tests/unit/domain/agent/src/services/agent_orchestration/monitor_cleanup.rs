//! `AgentMonitor::cleanup_unresponsive_agents`, which no test called.
//!
//! Reaching it needs an agent that is *alive but unresponsive*: `get_status`
//! downgrades a row whose pid is gone to `Failed` before the monitor ever sees
//! it, so `get_unresponsive_agents` only ever yields agents whose process
//! still exists. The fixture therefore registers this test process's own pid
//! against a dead port — alive, so the row stays `Running`, and unreachable,
//! so the card probe fails.

use std::collections::HashMap;

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::AgentStatus;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_agent::services::agent_orchestration::monitor::AgentMonitor;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_models::ServicesConfig;
use uuid::Uuid;

use super::super::a2a_server::a2a_helpers::agent_config;
use crate::repository::try_pool;

fn unique_name(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn db_service(pool: &systemprompt_database::DbPool, name: &str, port: u16) -> AgentDatabaseService {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let mut config = agent_config(name);
    config.port = port;
    let mut agents = HashMap::new();
    agents.insert(name.to_owned(), config);
    let registry = AgentRegistry::from_config(ServicesConfig {
        agents,
        ..ServicesConfig::default()
    });
    let repo = AgentServiceRepository::new(
        pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    )
    .expect("repo");
    AgentDatabaseService::with_registry(repo, registry)
}

fn dead_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    port
}

// Why: cleanup is what stops a wedged agent's row from blocking its own
// restart. The row must be reclaimed and the crash recorded.
//
// The pid registered here is the test runner's, which is deliberately not one
// of the orchestrator's children: `kill_process_verified` refuses to signal a
// pid it cannot claim, so the reclaim happens without the suite killing
// itself. That refusal is the same guard a recycled pid would hit in
// production.
#[tokio::test]
async fn cleanup_reclaims_an_unresponsive_agent_and_records_the_crash() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let name = unique_name("mon_cleanup");
    let port = dead_port();
    let svc = db_service(&pool, &name, port);
    svc.register_agent(&name, std::process::id(), port)
        .await
        .expect("register");

    let listed = svc.get_unresponsive_agents().await.expect("unresponsive");
    assert!(
        listed.iter().any(|(agent, _)| agent == &name),
        "the fixture agent must be unresponsive before cleanup runs"
    );

    let cleaned = AgentMonitor::with_db_service(db_service(&pool, &name, port))
        .cleanup_unresponsive_agents()
        .await
        .expect("cleanup");

    assert!(
        cleaned >= 1,
        "the fixture agent must be counted as cleaned up, got {cleaned}"
    );

    let status = svc.get_status(&name).await.expect("status after cleanup");
    assert!(
        matches!(status, AgentStatus::Failed { .. }),
        "a reclaimed agent must no longer read as Running: {status:?}"
    );

    svc.remove_agent_service(&name).await.ok();
}
