//! DB-backed tests for `FingerprintRepository`: reputation upsert semantics,
//! abuse flagging, the request counter, and the read queries over
//! `fingerprint_reputation` and `user_sessions`.

use systemprompt_analytics::FlagReason;
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
    let repo = systemprompt_test_fixtures::fixture_fingerprint_repository(&pool).expect("repo");

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

    cleanup(&pool, &fp).await;
}

#[tokio::test]
async fn flag_and_request_counter_persist() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = systemprompt_test_fixtures::fixture_fingerprint_repository(&pool).expect("repo");

    let fp = unique_fingerprint();
    repo.upsert_fingerprint(&fp, None, None, None)
        .await
        .expect("insert");

    repo.flag_fingerprint(&fp, FlagReason::HighRequestCount, 10)
        .await
        .expect("flag");
    repo.increment_request_count(&fp).await.expect("request");

    let (is_flagged, flag_reason, reputation_score, total_request_count): (
        bool,
        Option<String>,
        i32,
        i64,
    ) = sqlx::query_as(
        "SELECT is_flagged, flag_reason, reputation_score, total_request_count \
         FROM fingerprint_reputation WHERE fingerprint_hash = $1",
    )
    .bind(&fp)
    .fetch_one(pool.pool_arc().expect("pool").as_ref())
    .await
    .expect("row");
    assert!(is_flagged);
    assert_eq!(flag_reason.as_deref(), Some("request_count_exceeded_100"));
    assert_eq!(reputation_score, 10);
    assert_eq!(total_request_count, 2);

    cleanup(&pool, &fp).await;
}

#[tokio::test]
async fn session_queries_count_and_reuse_active_sessions() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = systemprompt_test_fixtures::fixture_fingerprint_repository(&pool).expect("repo");
    let sessions = systemprompt_test_fixtures::fixture_analytics_repositories(&pool)
        .map(|repositories| repositories.sessions)
        .expect("session repo");

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
    assert_eq!(reusable, sid);

    cleanup(&pool, &fp).await;
}
