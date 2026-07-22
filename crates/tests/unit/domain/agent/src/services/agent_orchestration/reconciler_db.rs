// DB-backed tests for AgentReconciler over an injected registry
// (with_db_service + AgentDatabaseService::with_registry), so the scan set is
// exactly the agents this test configures. get_status repairs dead-PID rows to
// Failed as a side effect, so the check buckets a dead-PID agent under
// `failed`, while fix_inconsistencies is driven with an explicit report.

use std::collections::HashMap;

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::AgentStatus;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_agent::services::agent_orchestration::reconciler::{
    AgentReconciler, ConsistencyReport,
};
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_models::ServicesConfig;
use uuid::Uuid;

use super::super::a2a_server::a2a_helpers::agent_config;
use crate::repository::try_pool;

const DEAD_PID: u32 = 4_000_000_001;

fn unique_name(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn db_service_with(
    pool: &systemprompt_database::DbPool,
    names_and_ports: &[(&str, u16)],
) -> AgentDatabaseService {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let mut agents = HashMap::new();
    for (name, port) in names_and_ports {
        let mut config = agent_config(name);
        config.port = *port;
        agents.insert((*name).to_owned(), config);
    }
    let registry = AgentRegistry::from_config(ServicesConfig {
        agents,
        ..ServicesConfig::default()
    });
    let repo = AgentServiceRepository::new(pool).expect("repo");
    AgentDatabaseService::with_registry(repo, registry)
}

#[tokio::test]
async fn reconcile_repairs_dead_pid_row_via_status_lookup() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let name = unique_name("rec_dead");
    let svc = db_service_with(&pool, &[(&name, 9410)]);
    svc.register_agent(&name, DEAD_PID, 9410)
        .await
        .expect("register");

    let reconciler = AgentReconciler::with_db_service(db_service_with(&pool, &[(&name, 9410)]));
    let reconciled = reconciler
        .reconcile_running_services()
        .await
        .expect("reconcile");
    assert_eq!(reconciled, 0);

    let status = svc.get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn consistency_check_buckets_live_and_dead_agents() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let dead = unique_name("rec_bucket_d");
    let live = unique_name("rec_bucket_l");
    let svc = db_service_with(&pool, &[(&dead, 9413), (&live, 9414)]);
    svc.register_agent(&dead, DEAD_PID, 9413)
        .await
        .expect("register dead");
    svc.register_agent(&live, std::process::id(), 9414)
        .await
        .expect("register live");

    let reconciler =
        AgentReconciler::with_db_service(db_service_with(&pool, &[(&dead, 9413), (&live, 9414)]));
    let report = reconciler
        .perform_consistency_check()
        .await
        .expect("consistency check");

    assert!(report.failed.iter().any(|agent| agent == &dead));
    assert!(report.consistent_running.iter().any(|agent| agent == &live));
    assert_eq!(report.total_agents(), 2);
    assert!(!report.has_inconsistencies());

    svc.remove_agent_service(&dead).await.ok();
    svc.remove_agent_service(&live).await.ok();
}

#[tokio::test]
async fn fix_inconsistencies_marks_reported_agents_failed() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let stale = unique_name("rec_fix_a");
    let orphan = unique_name("rec_fix_b");
    let svc = db_service_with(&pool, &[(&stale, 9415), (&orphan, 9416)]);
    svc.register_agent(&stale, std::process::id(), 9415)
        .await
        .expect("register stale");
    svc.register_agent(&orphan, std::process::id(), 9416)
        .await
        .expect("register orphan");

    let mut report = ConsistencyReport::new();
    report.inconsistent_running.push((stale.clone(), DEAD_PID));
    report.orphaned_processes.push((orphan.clone(), DEAD_PID));
    assert!(report.has_inconsistencies());

    let reconciler = AgentReconciler::with_db_service(db_service_with(
        &pool,
        &[(&stale, 9415), (&orphan, 9416)],
    ));
    let fixed = reconciler.fix_inconsistencies(&report).await.expect("fix");
    assert_eq!(fixed, 2);

    for name in [&stale, &orphan] {
        let status = svc.get_status(name).await.expect("status");
        assert!(matches!(status, AgentStatus::Failed { .. }));
    }

    svc.remove_agent_service(&stale).await.ok();
    svc.remove_agent_service(&orphan).await.ok();
}

#[test]
fn reconcile_starting_services_reports_zero() {
    assert_eq!(AgentReconciler::reconcile_starting_services(), 0);
}
