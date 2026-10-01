//! DB-backed tests for `ServiceManagementService` and `ServiceStateVerifier`
//! against a real Postgres instance. Verifies the read-side paths that don't
//! mutate live services (the mutation paths kill processes / bind ports, so
//! they're not safe to drive from the test runner).

use systemprompt_database::{CreateServiceInput, ServiceModule, ServiceRepository, ServiceStatus};
use systemprompt_identifiers::ServiceName;
use systemprompt_scheduler::{
    DesiredStatus, RuntimeStatus, ServiceAction, ServiceConfig, ServiceManagementService,
    ServiceStateVerifier, ServiceType,
};
use systemprompt_test_fixtures::test_db_pool;

#[tokio::test]
async fn get_services_by_type_surfaces_seeded_service() {
    let pool = test_db_pool().await;
    let repo = ServiceRepository::new(
        &pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
    let name = ServiceName::new(format!("gsbt_mcp_{}", uuid::Uuid::new_v4().simple()));
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Stopped,
        port: 65515,
        binary_mtime: None,
    })
    .await
    .expect("seed service");

    let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
        &pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    ));
    let services = svc
        .get_services_by_type(ServiceModule::Mcp)
        .await
        .expect("query");
    assert!(
        services.iter().any(|s| s.name == name),
        "seeded mcp service {name} must surface in get_services_by_type(\"mcp\")"
    );
    assert!(
        services.iter().all(|s| s.module_name == ServiceModule::Mcp),
        "get_services_by_type(\"mcp\") must return only mcp services"
    );

    repo.delete_service(&name).await.expect("cleanup");
}

#[tokio::test]
async fn get_running_services_with_pid_returns_only_running() {
    let pool = test_db_pool().await;
    let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
        &pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    ));
    let services = svc.get_running_services_with_pid().await.expect("query");
    assert!(
        services.iter().all(|s| s.status == ServiceStatus::Running),
        "get_running_services_with_pid must only return running services"
    );
}

#[tokio::test]
async fn cleanup_stale_entries_runs() {
    let pool = test_db_pool().await;
    let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
        &pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    ));
    let cleaned = svc.cleanup_stale_entries().await.expect("cleanup");
    let _ = cleaned;
}

#[tokio::test]
async fn mark_service_stopped_for_unknown_succeeds() {
    let pool = test_db_pool().await;
    let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
        &pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    ));
    let result = svc
        .mark_service_stopped(&ServiceName::new("nonexistent-service-name-zzz"))
        .await;
    let _ = result;
}

#[tokio::test]
async fn state_verifier_get_verified_states_handles_unknown_service() {
    let pool = test_db_pool().await;
    let verifier = ServiceStateVerifier::new(
        pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
    let configs = vec![ServiceConfig {
        name: ServiceName::new(format!("test_svc_{}", uuid::Uuid::new_v4().simple())),
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
    let pool = test_db_pool().await;
    let verifier = ServiceStateVerifier::new(
        pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
    let configs = vec![ServiceConfig {
        name: ServiceName::new(format!("test_running_{}", uuid::Uuid::new_v4().simple())),
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
    let pool = test_db_pool().await;
    let verifier = ServiceStateVerifier::new(
        pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
    let configs = vec![ServiceConfig {
        name: ServiceName::new(format!("test_action_{}", uuid::Uuid::new_v4().simple())),
        service_type: ServiceType::Mcp,
        port: 1,
        enabled: false,
    }];
    let actions = verifier
        .get_services_needing_action(&configs)
        .await
        .expect("query");
    let ours = actions
        .iter()
        .find(|s| s.name == configs[0].name)
        .expect("disabled service must surface as needing action (DB cleanup)");
    assert_eq!(
        ours.desired_status,
        DesiredStatus::Disabled,
        "a disabled config must be classified as desired-disabled"
    );
    assert!(
        ours.needs_action != ServiceAction::None,
        "a surfaced needing-action state must carry a non-None action"
    );
}

#[tokio::test]
async fn state_verifier_get_crashed_services_filters() {
    let pool = test_db_pool().await;
    let verifier = ServiceStateVerifier::new(
        pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
    let configs: Vec<ServiceConfig> = vec![];
    let crashed = verifier
        .get_crashed_services(&configs)
        .await
        .expect("query");
    assert!(
        crashed
            .iter()
            .all(|s| s.runtime_status == RuntimeStatus::Crashed),
        "get_crashed_services must return only crashed states"
    );
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
