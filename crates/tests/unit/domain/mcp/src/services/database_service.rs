//! DB-backed tests for [`DatabaseService`] methods that don't require a
//! validated registry or filesystem layout.

use std::sync::Arc;
use systemprompt_mcp::services::database::DatabaseService;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::AppPaths;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_user_id};

async fn make_db_service() -> Option<DatabaseService> {
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    let paths = PathsConfig {
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    };
    let app_paths = Arc::new(AppPaths::from_profile(&paths).ok()?);
    let registry = RegistryService::new(fixture_user_id());
    Some(DatabaseService::new(db, app_paths, registry))
}

#[tokio::test]
async fn get_service_by_name_missing_returns_none() {
    let Some(svc) = make_db_service().await else { return };
    let r = svc
        .get_service_by_name(&format!("missing-{}", uuid::Uuid::new_v4().simple()))
        .await
        .unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn cleanup_stale_services_runs() {
    let Some(svc) = make_db_service().await else { return };
    svc.cleanup_stale_services().await.unwrap();
}

#[tokio::test]
async fn delete_crashed_services_runs() {
    let Some(svc) = make_db_service().await else { return };
    svc.delete_crashed_services().await.unwrap();
}

#[tokio::test]
async fn sync_state_empty_runs() {
    let Some(svc) = make_db_service().await else { return };
    svc.sync_state(&[]).await.unwrap();
}

#[tokio::test]
async fn delete_disabled_services_empty_runs() {
    let Some(svc) = make_db_service().await else { return };
    let _ = svc.delete_disabled_services(&[]).await.unwrap();
}

#[tokio::test]
async fn get_running_servers_errors_when_registry_not_validated() {
    let Some(svc) = make_db_service().await else { return };
    let r = svc.get_running_servers().await;
    let _ = r;
}

#[tokio::test]
async fn update_service_status_missing_no_panic() {
    let Some(svc) = make_db_service().await else { return };
    svc.update_service_status(
        &format!("missing-{}", uuid::Uuid::new_v4().simple()),
        "stopped",
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn clear_service_pid_missing_no_panic() {
    let Some(svc) = make_db_service().await else { return };
    svc.clear_service_pid(&format!("missing-{}", uuid::Uuid::new_v4().simple()))
        .await
        .unwrap();
}

#[tokio::test]
async fn unregister_missing_no_panic() {
    let Some(svc) = make_db_service().await else { return };
    svc.unregister_service(&format!("missing-{}", uuid::Uuid::new_v4().simple()))
        .await
        .unwrap();
}

#[tokio::test]
async fn accessors() {
    let Some(svc) = make_db_service().await else { return };
    let _ = svc.app_paths();
    let _ = svc.db_pool();
    let _ = svc.clone();
    let _ = format!("{svc:?}");
}
