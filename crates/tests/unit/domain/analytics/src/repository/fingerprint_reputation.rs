//! DB-backed tests for `FingerprintRepository`: reputation upsert semantics,
//! velocity/session-count updates, abuse flagging and clearing, bounded
//! reputation-score adjustment, and the read queries over
//! `fingerprint_reputation` and `user_sessions`.

use systemprompt_analytics::{FingerprintRepository, FlagReason, SessionRepository};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

use super::session_support::{seed_session, unique_session_id};

fn unique_fingerprint() -> String {
    format!("fp-{}", Uuid::new_v4())
}

async fn cleanup(pool: &DbPool, fingerprint: &str) {
    let p = pool.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM user_sessions WHERE fingerprint_hash = $1")
        .bind(fingerprint)
        .execute(p.as_ref())
        .await
        .ok();
    sqlx::query("DELETE FROM fingerprint_reputation WHERE fingerprint_hash = $1")
        .bind(fingerprint)
        .execute(p.as_ref())
        .await
        .ok();
}

#[tokio::test]
async fn upsert_fingerprint_inserts_then_accumulates() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FingerprintRepository::new(&pool).expect("repo");

    let fp = unique_fingerprint();
    let user = UserId::new(format!("user-{}", Uuid::new_v4()));

    let first = repo
        .upsert_fingerprint(&fp, Some("10.0.0.1"), Some("agent/1"), None)
        .await
        .expect("insert");
    assert_eq!(first.total_session_count, 1);
    assert!(first.associated_user_ids.is_empty());
    assert_eq!(first.last_ip_address.as_deref(), Some("10.0.0.1"));

    let second = repo
        .upsert_fingerprint(&fp, None, None, Some(&user))
        .await
        .expect("upsert");
    assert_eq!(second.total_session_count, 2);
    assert_eq!(second.associated_user_ids, vec![user.as_str().to_owned()]);
    assert_eq!(second.last_ip_address.as_deref(), Some("10.0.0.1"));

    let third = repo
        .upsert_fingerprint(&fp, Some("10.0.0.2"), None, Some(&user))
        .await
        .expect("upsert same user");
    assert_eq!(third.total_session_count, 3);
    assert_eq!(third.associated_user_ids.len(), 1);
    assert_eq!(third.last_ip_address.as_deref(), Some("10.0.0.2"));

    let fetched = repo
        .get_by_hash(&fp)
        .await
        .expect("get_by_hash")
        .expect("present");
    assert_eq!(fetched.total_session_count, 3);

    assert!(
        repo.get_by_hash("no-such-fp")
            .await
            .expect("miss")
            .is_none()
    );

    cleanup(&pool, &fp).await;
}

#[tokio::test]
async fn flag_clear_and_score_adjustment_round_trip() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FingerprintRepository::new(&pool).expect("repo");

    let fp = unique_fingerprint();
    repo.upsert_fingerprint(&fp, None, None, None)
        .await
        .expect("insert");

    repo.flag_fingerprint(&fp, FlagReason::HighRequestCount, 10)
        .await
        .expect("flag");
    let flagged = repo.get_by_hash(&fp).await.expect("get").expect("present");
    assert!(flagged.is_flagged);
    assert!(flagged.flag_reason.is_some());
    assert!(flagged.flagged_at.is_some());
    assert_eq!(flagged.reputation_score, 10);

    repo.clear_flag(&fp).await.expect("clear");
    let cleared = repo.get_by_hash(&fp).await.expect("get").expect("present");
    assert!(!cleared.is_flagged);
    assert!(cleared.flag_reason.is_none());

    let raised = repo.adjust_reputation_score(&fp, 500).await.expect("raise");
    assert_eq!(raised, 100);
    let floored = repo
        .adjust_reputation_score(&fp, -500)
        .await
        .expect("floor");
    assert_eq!(floored, 0);

    cleanup(&pool, &fp).await;
}

#[tokio::test]
async fn velocity_and_request_counters_update() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FingerprintRepository::new(&pool).expect("repo");

    let fp = unique_fingerprint();
    repo.upsert_fingerprint(&fp, None, None, None)
        .await
        .expect("insert");

    repo.update_velocity_metrics(&fp, 42, 7.5, 3)
        .await
        .expect("velocity");
    repo.increment_request_count(&fp).await.expect("request");
    repo.update_active_session_count(&fp, 4)
        .await
        .expect("active count");

    let row = repo.get_by_hash(&fp).await.expect("get").expect("present");
    assert_eq!(row.requests_last_hour, 42);
    assert!((row.peak_requests_per_minute - 7.5).abs() < f32::EPSILON);
    assert_eq!(row.sustained_high_velocity_minutes, 3);
    assert_eq!(row.total_request_count, 2);
    assert_eq!(row.active_session_count, 4);

    cleanup(&pool, &fp).await;
}

#[tokio::test]
async fn session_queries_count_and_reuse_active_sessions() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FingerprintRepository::new(&pool).expect("repo");
    let sessions = SessionRepository::new(&pool).expect("session repo");

    let fp = unique_fingerprint();
    assert_eq!(
        repo.count_active_sessions(&fp).await.expect("count empty"),
        0
    );
    assert!(
        repo.find_reusable_session(&fp)
            .await
            .expect("reuse empty")
            .is_none()
    );

    let sid = unique_session_id();
    seed_session(&sessions, &sid, &fp).await;

    assert_eq!(repo.count_active_sessions(&fp).await.expect("count"), 1);
    let reusable = repo
        .find_reusable_session(&fp)
        .await
        .expect("reuse")
        .expect("present");
    assert_eq!(reusable, sid.as_str());

    cleanup(&pool, &fp).await;
}
