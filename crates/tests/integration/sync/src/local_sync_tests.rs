//! DB-backed integration tests for `ContentLocalSync` and
//! `AccessControlLocalSync` error/happy paths that the wiremock-only suite
//! cannot reach.

use std::path::PathBuf;
use systemprompt_sync::{AccessControlLocalSync, ContentLocalSync};
use tempfile::TempDir;

#[tokio::test]
async fn content_local_sync_calculate_diff_against_empty_dir_returns_empty_result() {
    let Some(db) = crate::support::try_db().await else {
        return;
    };
    let sync = ContentLocalSync::new(db);
    let dir = TempDir::new().expect("tempdir");
    let source = systemprompt_identifiers::SourceId::new(format!("src-{}", uuid::Uuid::new_v4()));
    let result = sync
        .calculate_diff(&source, dir.path(), &["article".to_owned()])
        .await
        .expect("calculate_diff on empty dir");
    assert!(
        result.added.is_empty() && result.modified.is_empty(),
        "empty dir + unique source should yield empty diff; got added={:?} modified={:?}",
        result.added,
        result.modified
    );
}

#[tokio::test]
async fn access_control_local_sync_missing_yaml_path_returns_missing_config() {
    let Some(db) = crate::support::try_db().await else {
        return;
    };
    let missing = PathBuf::from("/no/such/file/sync-test.yaml");
    let sync = AccessControlLocalSync::new(db, missing);
    let err = sync
        .sync_to_db(false, false)
        .await
        .expect_err("missing file must error");
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("missing") || msg.to_lowercase().contains("not found"),
        "expected missing-config error, got: {msg}"
    );
}

#[tokio::test]
async fn access_control_local_sync_invalid_yaml_returns_invalid_input() {
    let Some(db) = crate::support::try_db().await else {
        return;
    };
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("acl.yaml");
    std::fs::write(&path, "this:\n  is: not\n  ::valid yaml@@@@").expect("write");
    let sync = AccessControlLocalSync::new(db, path);
    let err = sync
        .sync_to_db(false, false)
        .await
        .expect_err("invalid yaml must error");
    let msg = err.to_string();
    assert!(!msg.is_empty(), "expected non-empty error for invalid yaml");
}

#[tokio::test]
async fn access_control_local_sync_empty_config_succeeds_with_zero_synced() {
    let Some(db) = crate::support::try_db().await else {
        return;
    };
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("acl.yaml");
    std::fs::write(&path, "rules: []\n").expect("write");
    let sync = AccessControlLocalSync::new(db, path);
    let result = sync
        .sync_to_db(false, false)
        .await
        .expect("empty config ok");
    assert_eq!(result.items_synced, 0);
    assert_eq!(result.items_deleted, 0);
    assert!(result.errors.is_empty());
}
