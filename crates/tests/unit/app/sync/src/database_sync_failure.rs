//! Exercise `DatabaseSyncService::sync` on a deliberately-bad URL so the
//! error path runs without needing a live Postgres.

use systemprompt_sync::{DatabaseSyncService, SyncDirection};

#[tokio::test]
async fn push_with_unreachable_db_returns_error() {
    let service = DatabaseSyncService::new(
        SyncDirection::Push,
        false,
        "postgres://nouser:nopass@127.0.0.1:1/nodb",
        "postgres://nouser:nopass@127.0.0.1:1/nodb",
    );
    let result = service.sync().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn pull_with_unreachable_db_returns_error() {
    let service = DatabaseSyncService::new(
        SyncDirection::Pull,
        false,
        "postgres://nouser:nopass@127.0.0.1:1/nodb",
        "postgres://nouser:nopass@127.0.0.1:1/nodb",
    );
    let result = service.sync().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn push_dry_run_with_unreachable_db_still_errors_on_export() {
    let service = DatabaseSyncService::new(
        SyncDirection::Push,
        true,
        "postgres://nouser:nopass@127.0.0.1:1/nodb",
        "postgres://nouser:nopass@127.0.0.1:1/nodb",
    );
    let result = service.sync().await;
    assert!(result.is_err());
}

#[test]
fn service_debug_includes_direction_and_dry_run() {
    let service = DatabaseSyncService::new(
        SyncDirection::Pull,
        true,
        "postgres://local",
        "postgres://cloud",
    );
    let dbg = format!("{service:?}");
    assert!(dbg.contains("Pull"));
    assert!(dbg.contains("dry_run: true"));
}
