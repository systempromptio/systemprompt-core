//! `SyncService::sync_all` aggregation across a succeeding file step and a
//! failing database step.
//!
//! A dry-run push over an empty services directory lets `sync_files` succeed
//! without touching the network, while an absent `local_database_url` makes
//! `sync_database` fail with `MissingConfig`. `sync_all` must still return
//! `Ok`, folding the database failure into a per-operation result whose state
//! is `NotStarted` — this exercises the `database_failure_result` mapping in
//! `lib.rs` that the network-dependent happy path never reaches in unit tests.

use systemprompt_identifiers::TenantId;
use systemprompt_sync::{SyncConfig, SyncDirection, SyncOpState, SyncService};
use tempfile::TempDir;

#[tokio::test]
async fn sync_all_records_not_started_database_when_local_url_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let cfg = SyncConfig::builder(
        TenantId::new("tenant-sync-all"),
        "https://api.example.com",
        "tok",
        tmp.path().to_str().expect("utf8 services path"),
    )
    .with_direction(SyncDirection::Push)
    .with_dry_run(true)
    .build();

    let service = SyncService::new(cfg).expect("service");
    let results = service
        .sync_all()
        .await
        .expect("sync_all folds the db failure into a result rather than erroring");

    let files = results
        .iter()
        .find(|r| r.operation == "files_push")
        .expect("files operation present");
    assert!(files.success, "dry-run file push should succeed");

    let database = results
        .iter()
        .find(|r| r.operation == "database")
        .expect("database operation present");
    assert!(!database.success, "database step should be marked failed");
    assert_eq!(database.state, SyncOpState::NotStarted);
    assert!(
        database
            .errors
            .iter()
            .any(|e| e.contains("local_database_url")),
        "database error should name the missing config: {:?}",
        database.errors
    );
}
