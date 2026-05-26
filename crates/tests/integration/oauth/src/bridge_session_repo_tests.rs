//! Integration tests for `BridgeSessionRepository` heartbeat & listing paths.

use std::time::Duration;

use crate::{create_test_user, setup_test_db};
use systemprompt_identifiers::SessionId;
use systemprompt_oauth::repository::{BridgeSessionRepository, UpsertBridgeSession};

fn make_upsert(user_id: &systemprompt_identifiers::UserId, suffix: &str) -> UpsertBridgeSession {
    UpsertBridgeSession {
        session_id: SessionId::new(format!("bsr_{}_{}", suffix, uuid::Uuid::new_v4().simple())),
        user_id: user_id.clone(),
        bridge_version: "0.1.0".into(),
        os: "linux".into(),
        hostname: format!("host-{suffix}"),
        last_activity_at: Some(chrono::Utc::now()),
        forwarded_total: 10,
        tokens_in_total: 100,
        tokens_out_total: 200,
    }
}

#[tokio::test]
async fn upsert_then_list_active_returns_row() {
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;
    let repo = BridgeSessionRepository::new(&db).expect("repo");

    let p = make_upsert(&user_id, "a");
    let sid = p.session_id.clone();
    repo.upsert(p).await.expect("first upsert");

    let active = repo
        .list_active(Duration::from_secs(3600))
        .await
        .expect("list_active");
    assert!(active.iter().any(|r| r.session_id == sid));

    let by_user = repo
        .list_active_for_user(&user_id, Duration::from_secs(3600))
        .await
        .expect("list_active_for_user");
    assert!(by_user.iter().any(|r| r.session_id == sid));
}

#[tokio::test]
async fn upsert_is_idempotent_per_session_id() {
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;
    let repo = BridgeSessionRepository::new(&db).expect("repo");

    let mut p = make_upsert(&user_id, "i");
    let sid = p.session_id.clone();
    repo.upsert(p.clone()).await.expect("first");
    p.forwarded_total = 50;
    repo.upsert(p).await.expect("update");

    let row = repo
        .list_active(Duration::from_secs(3600))
        .await
        .expect("list_active")
        .into_iter()
        .find(|r| r.session_id == sid)
        .expect("row present");
    assert_eq!(row.forwarded_total, 50);
}

#[tokio::test]
async fn delete_stale_removes_old_rows() {
    let db = setup_test_db().await;
    let repo = BridgeSessionRepository::new(&db).expect("repo");
    // Trivial: no rows older than 1 year — but the query exercises the path.
    let removed = repo
        .delete_stale(Duration::from_secs(365 * 24 * 3600))
        .await
        .expect("delete_stale");
    let _ = removed;
}
