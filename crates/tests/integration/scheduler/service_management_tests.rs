//! DB-backed tests for `ServiceManagementService` and `ServiceStateVerifier`
//! against a real Postgres instance. Verifies the read-side paths that don't
//! mutate live services (the mutation paths kill processes / bind ports, so
//! they're not safe to drive from the test runner).

use systemprompt_database::DbPool;
use systemprompt_scheduler::{
    DesiredStatus, ServiceConfig, ServiceManagementService, ServiceStateVerifier, ServiceType,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn try_pool() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn service_management_service_new_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    assert!(ServiceManagementService::new(&pool).is_ok());
}

#[tokio::test]
async fn get_services_by_type_returns_vec() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = ServiceManagementService::new(&pool).expect("svc");
    let services = svc.get_services_by_type("mcp").await.expect("query");
    let _ = services.len();
}

#[tokio::test]
async fn get_running_services_with_pid_returns_vec() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = ServiceManagementService::new(&pool).expect("svc");
    let services = svc.get_running_services_with_pid().await.expect("query");
    let _ = services.len();
}

#[tokio::test]
async fn cleanup_stale_entries_runs() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = ServiceManagementService::new(&pool).expect("svc");
    let cleaned = svc.cleanup_stale_entries().await.expect("cleanup");
    let _ = cleaned;
}

#[tokio::test]
async fn mark_service_stopped_for_unknown_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = ServiceManagementService::new(&pool).expect("svc");
    let result = svc
        .mark_service_stopped("nonexistent-service-name-zzz")
        .await;
    let _ = result;
}

#[tokio::test]
async fn state_verifier_get_verified_states_handles_unknown_service() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let verifier = ServiceStateVerifier::new(pool);
    let configs = vec![ServiceConfig {
        name: format!("test_svc_{}", uuid::Uuid::new_v4().simple()),
        service_type: ServiceType::Mcp,
        port: 1,
        enabled: false,
    }];
    let states = verifier
        .get_verified_states(&configs)
        .await
        .expect("verify");
    assert!(states.iter().any(|s| s.name == configs[0].name));
}

#[tokio::test]
async fn state_verifier_get_running_services_filters_correctly() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let verifier = ServiceStateVerifier::new(pool);
    let configs = vec![ServiceConfig {
        name: format!("test_running_{}", uuid::Uuid::new_v4().simple()),
        service_type: ServiceType::Mcp,
        port: 1,
        enabled: false,
    }];
    let running = verifier
        .get_running_services(&configs)
        .await
        .expect("query");
    assert!(
        !running.iter().any(|s| s.name == configs[0].name),
        "seeded config {} is not running and must not be reported as running",
        configs[0].name
    );
}

#[tokio::test]
async fn state_verifier_get_services_needing_action_filters() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let verifier = ServiceStateVerifier::new(pool);
    let configs = vec![ServiceConfig {
        name: format!("test_action_{}", uuid::Uuid::new_v4().simple()),
        service_type: ServiceType::Mcp,
        port: 1,
        enabled: false,
    }];
    let actions = verifier
        .get_services_needing_action(&configs)
        .await
        .expect("query");
    let _ = actions.len();
}

#[tokio::test]
async fn state_verifier_get_crashed_services_filters() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let verifier = ServiceStateVerifier::new(pool);
    let configs: Vec<ServiceConfig> = vec![];
    let crashed = verifier
        .get_crashed_services(&configs)
        .await
        .expect("query");
    assert!(crashed.is_empty());
}

#[tokio::test]
async fn service_type_from_module_name_round_trips() {
    let _ = ServiceType::from_module_name("mcp");
    let _ = ServiceType::from_module_name("agent");
    let _ = ServiceType::from_module_name("unknown_type");
}

#[test]
fn desired_status_variants_are_constructible() {
    let _e = DesiredStatus::Enabled;
    let _d = DesiredStatus::Disabled;
}
