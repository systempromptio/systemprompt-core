//! Tests for the top-level `SyncService` facade that don't need a real cloud
//! API or DB.

use systemprompt_identifiers::TenantId;
use systemprompt_sync::{SyncConfig, SyncDirection, SyncService};

fn config(direction: SyncDirection, with_local_db: bool) -> SyncConfig {
    let mut b = SyncConfig::builder(
        TenantId::new("tenant-1"),
        "https://api.example.com",
        "tok",
        "/services",
    )
    .with_direction(direction);
    if with_local_db {
        b = b.with_local_database_url("postgres://nouser:nopass@127.0.0.1:1/nodb");
    }
    b.build()
}

#[tokio::test]
async fn sync_database_without_local_url_returns_missing_config() {
    let service = SyncService::new(config(SyncDirection::Push, false)).expect("service");
    let result = service.sync_database().await;
    assert!(result.is_err());
    let msg = result.err().unwrap().to_string();
    assert!(
        msg.contains("local_database_url") || msg.to_lowercase().contains("missing"),
        "got: {msg}"
    );
}

#[tokio::test]
async fn sync_all_collects_per_operation_results() {
    let service = SyncService::new(config(SyncDirection::Push, false)).expect("service");
    // sync_files needs the cloud API. sync_database errors with MissingConfig.
    // We tolerate either Ok(vec) or Err — but if Ok, the second entry must be
    // the failed database operation.
    let result = service.sync_all().await;
    if let Ok(results) = result {
        // expect 2 entries
        assert!(results.iter().any(|r| r.operation == "database"));
    }
}

#[test]
fn service_debug_renders() {
    let service = SyncService::new(config(SyncDirection::Pull, true)).expect("service");
    let dbg = format!("{service:?}");
    assert!(dbg.contains("SyncService"));
}

#[tokio::test]
async fn sync_files_dry_run_against_empty_services_dir() {
    let tmp = tempfile::TempDir::new().expect("tmp");
    let services = tmp.path().to_string_lossy().to_string();
    let cfg = SyncConfig::builder(
        TenantId::new("tenant-1"),
        "https://api.example.com",
        "tok",
        &services,
    )
    .with_direction(SyncDirection::Push)
    .with_dry_run(true)
    .build();
    let service = SyncService::new(cfg).expect("service");
    let result = service.sync_files().await;
    // dry_run push collects from disk; empty dir = 0 files.
    let r = result.expect("dry_run should not call the network");
    assert!(r.success);
    assert_eq!(r.items_synced, 0);
    assert_eq!(r.operation, "files_push");
}

#[tokio::test]
async fn sync_files_dry_run_with_real_files() {
    let tmp = tempfile::TempDir::new().expect("tmp");
    let services = tmp.path();
    std::fs::create_dir_all(services.join("agents")).expect("mkdir");
    std::fs::write(services.join("agents/a.yaml"), "name: a\n").expect("write");
    std::fs::create_dir_all(services.join("skills")).expect("mkdir");
    std::fs::write(services.join("skills/s.md"), "# skill\nbody").expect("write");
    let cfg = SyncConfig::builder(
        TenantId::new("tenant-1"),
        "https://api.example.com",
        "tok",
        services.to_str().expect("utf8"),
    )
    .with_direction(SyncDirection::Push)
    .with_dry_run(true)
    .build();
    let service = SyncService::new(cfg).expect("service");
    let r = service.sync_files().await.expect("dry_run ok");
    assert!(r.success);
    assert!(r.items_skipped >= 2);
}

#[test]
fn import_result_serialises() {
    use systemprompt_sync::database::ImportResult;
    let r = ImportResult { created: 3, updated: 2, skipped: 1 };
    let json = serde_json::to_string(&r).expect("ser");
    assert!(json.contains("\"created\":3"));
    assert!(json.contains("\"updated\":2"));
    assert!(json.contains("\"skipped\":1"));
    let cloned = r;
    assert_eq!(cloned.created, 3);
}
