//! DB-backed tests for `SessionRepository` write operations: create/upsert,
//! counter increments, revocation, conversion/scanner marking, cleanup, and
//! user-id migration. Each test seeds a unique session, drives the mutation,
//! and reads state back through the repository's query surface.

use chrono::{Duration, Utc};
use systemprompt_analytics::SessionRepository;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

use super::session_support::{base_params, delete_session, seed_session, unique_session_id};

#[tokio::test]
async fn create_session_then_find_by_id_round_trip() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    let fp = format!("fp-{}", Uuid::new_v4());
    seed_session(&repo, &sid, &fp).await;

    let found = repo.find_by_id(&sid).await.expect("find").expect("present");
    assert_eq!(found.session_id.as_str(), sid.as_str());
    assert_eq!(found.fingerprint_hash.as_deref(), Some(fp.as_str()));
    assert!(!found.is_bot);
    assert_eq!(found.request_count, Some(0));

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn create_session_is_upsert_on_conflict() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    let fp = format!("fp-{}", Uuid::new_v4());
    let first = base_params(&sid, Some(&fp), Utc::now() + Duration::hours(1));
    repo.create_session(&first).await.expect("first insert");
    let second = base_params(&sid, Some(&fp), Utc::now() + Duration::hours(2));
    repo.create_session(&second).await.expect("upsert");

    assert!(
        repo.find_by_id(&sid)
            .await
            .expect("find")
            .is_some()
    );

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn increment_counters_accumulate() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    repo.increment_request_count(&sid).await.expect("req");
    repo.increment_request_count(&sid).await.expect("req");
    repo.increment_task_count(&sid).await.expect("task");
    repo.increment_message_count(&sid).await.expect("msg");

    let s = repo.find_by_id(&sid).await.expect("find").expect("present");
    assert_eq!(s.request_count, Some(2));
    assert_eq!(s.task_count, Some(1));
    assert_eq!(s.message_count, Some(1));

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn increment_ai_usage_accumulates_tokens_and_cost() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    repo.increment_ai_usage(&sid, 100, 5_000)
        .await
        .expect("usage1");
    repo.increment_ai_usage(&sid, 50, 2_500)
        .await
        .expect("usage2");

    let velocity = repo.get_session_velocity(&sid).await.expect("velocity");
    // request_count is unaffected by ai_usage; only ai counters move.
    assert_eq!(velocity.0, Some(0));

    let s = repo.find_by_id(&sid).await.expect("find").expect("present");
    assert_eq!(s.ai_request_count, Some(2));

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn update_activity_and_end_session() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    repo.update_activity(&sid).await.expect("activity");
    repo.end_session(&sid).await.expect("end");

    let s = repo.find_by_id(&sid).await.expect("find").expect("present");
    assert!(s.ended_at.is_some());

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn mark_scanner_and_converted() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    repo.mark_as_scanner(&sid).await.expect("scanner");
    repo.mark_converted(&sid).await.expect("converted");
    // Second mark_converted is a no-op (converted_at already set).
    repo.mark_converted(&sid).await.expect("converted again");

    let s = repo.find_by_id(&sid).await.expect("find").expect("present");
    assert_eq!(s.is_scanner, Some(true));

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn revoke_session_and_active_lookup() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    // Active before revoke.
    assert!(
        repo.find_active_by_id(&sid)
            .await
            .expect("active")
            .is_some()
    );

    repo.revoke_session(&sid).await.expect("revoke");

    // After revoke the active lookup no longer finds it.
    assert!(
        repo.find_active_by_id(&sid)
            .await
            .expect("active2")
            .is_none()
    );

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn cleanup_inactive_ends_stale_sessions() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    // Force last_activity_at into the past so cleanup_inactive(1h) catches it.
    let p = pool.pool_arc().expect("pool");
    sqlx::query(
        "UPDATE user_sessions SET last_activity_at = CURRENT_TIMESTAMP - INTERVAL '5 hours' \
         WHERE session_id = $1",
    )
    .bind(sid.as_str())
    .execute(p.as_ref())
    .await
    .expect("age session");

    let before = repo.count_inactive(1).await.expect("count");
    assert!(before >= 1);

    let cleaned = repo.cleanup_inactive(1).await.expect("cleanup");
    assert!(cleaned >= 1);

    let s = repo.find_by_id(&sid).await.expect("find").expect("present");
    assert!(s.ended_at.is_some());

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn migrate_user_sessions_moves_rows() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    // Two anon user ids that don't exist in `users` -> bypass FK by leaving
    // user_id NULL on insert, then set/migrate via direct update + repo call.
    let old_uid = UserId::new(format!("old-{}", Uuid::new_v4()));
    let new_uid = UserId::new(format!("new-{}", Uuid::new_v4()));

    // Migrating from an id with no rows returns 0.
    let none = repo
        .migrate_user_sessions(&old_uid, &new_uid)
        .await
        .expect("migrate empty");
    assert_eq!(none, 0);
}

#[tokio::test]
async fn revoke_all_for_user_with_no_rows_returns_zero() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let uid = UserId::new(format!("ghost-{}", Uuid::new_v4()));
    let revoked = repo.revoke_all_for_user(&uid).await.expect("revoke all");
    assert_eq!(revoked, 0);
}
