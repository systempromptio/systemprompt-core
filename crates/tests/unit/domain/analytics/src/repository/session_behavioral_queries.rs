//! DB-backed tests for `SessionRepository` behavioural read queries plus the
//! behavioural-detection writes. Sessions, analytics events and engagement
//! events are seeded with unique ids, then the windowed aggregates and
//! sequence/timestamp readers are asserted against known expected values.

use chrono::{Duration, Utc};
use systemprompt_analytics::SessionRepository;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

use super::session_support::{
    base_params, delete_session, insert_analytics_event, insert_engagement_event, seed_session,
    unique_session_id,
};

#[tokio::test]
async fn count_sessions_by_fingerprint_counts_within_window() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let fp = format!("fp-{}", Uuid::new_v4());
    let s1 = unique_session_id();
    let s2 = unique_session_id();
    seed_session(&repo, &s1, &fp).await;
    seed_session(&repo, &s2, &fp).await;

    let count = repo
        .count_sessions_by_fingerprint(&fp, 24)
        .await
        .expect("count");
    assert_eq!(count, 2);

    // A different fingerprint sees none.
    let other = repo
        .count_sessions_by_fingerprint(&format!("fp-{}", Uuid::new_v4()), 24)
        .await
        .expect("count other");
    assert_eq!(other, 0);

    delete_session(&pool, &s1).await;
    delete_session(&pool, &s2).await;
}

#[tokio::test]
async fn endpoint_sequence_and_timestamps_ordered() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    let now = Utc::now();
    insert_analytics_event(
        &pool,
        &sid,
        "page_view",
        Some("/a"),
        now - Duration::seconds(30),
    )
    .await;
    insert_analytics_event(
        &pool,
        &sid,
        "page_view",
        Some("/b"),
        now - Duration::seconds(20),
    )
    .await;
    // A non page_view event must not appear in the endpoint sequence.
    insert_analytics_event(
        &pool,
        &sid,
        "click",
        Some("/c"),
        now - Duration::seconds(10),
    )
    .await;

    let seq = repo.get_endpoint_sequence(&sid).await.expect("sequence");
    assert_eq!(seq, vec!["/a".to_owned(), "/b".to_owned()]);

    // Timestamps query returns all three events, ascending.
    let ts = repo.get_request_timestamps(&sid).await.expect("timestamps");
    assert_eq!(ts.len(), 3);
    assert!(ts[0] <= ts[1] && ts[1] <= ts[2]);

    assert!(repo.has_analytics_events(&sid).await.expect("has events"));

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn has_analytics_events_false_without_events() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    assert!(!repo.has_analytics_events(&sid).await.expect("none"));
    let empty = repo.get_endpoint_sequence(&sid).await.expect("empty seq");
    assert!(empty.is_empty());

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn session_for_behavioral_analysis_round_trip() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    let fp = format!("fp-{}", Uuid::new_v4());
    seed_session(&repo, &sid, &fp).await;
    repo.increment_request_count(&sid).await.expect("req");

    let data = repo
        .get_session_for_behavioral_analysis(&sid)
        .await
        .expect("query")
        .expect("present");
    assert_eq!(data.session_id.as_str(), sid.as_str());
    assert_eq!(data.fingerprint_hash.as_deref(), Some(fp.as_str()));
    assert_eq!(data.request_count, Some(1));

    // Missing session -> None.
    let missing = unique_session_id();
    assert!(
        repo.get_session_for_behavioral_analysis(&missing)
            .await
            .expect("missing")
            .is_none()
    );

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn count_unique_ips_by_fingerprint() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let fp = format!("fp-{}", Uuid::new_v4());
    for ip in ["1.1.1.1", "2.2.2.2", "1.1.1.1"] {
        let sid = unique_session_id();
        let mut params = base_params(&sid, Some(&fp), Utc::now() + Duration::hours(1));
        params.ip_address = Some(ip);
        repo.create_session(&params).await.expect("seed ip session");
    }

    let unique = repo
        .count_unique_ips_by_fingerprint(&fp, 7)
        .await
        .expect("unique ips");
    assert_eq!(unique, 2);

    let p = pool.pool_arc().expect("pool");
    sqlx::query("DELETE FROM user_sessions WHERE fingerprint_hash = $1")
        .bind(&fp)
        .execute(p.as_ref())
        .await
        .ok();
}

#[tokio::test]
async fn count_engagement_events_by_fingerprint() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let fp = format!("fp-{}", Uuid::new_v4());
    let sid = unique_session_id();
    seed_session(&repo, &sid, &fp).await;
    insert_engagement_event(&pool, &sid).await;
    insert_engagement_event(&pool, &sid).await;

    let count = repo
        .count_engagement_events_by_fingerprint(&fp, 7)
        .await
        .expect("engagement count");
    assert_eq!(count, 2);

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn session_starts_by_fingerprint_ordered() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let fp = format!("fp-{}", Uuid::new_v4());
    let s1 = unique_session_id();
    let s2 = unique_session_id();
    seed_session(&repo, &s1, &fp).await;
    seed_session(&repo, &s2, &fp).await;

    let starts = repo
        .get_session_starts_by_fingerprint(&fp, 7)
        .await
        .expect("starts");
    assert_eq!(starts.len(), 2);
    assert!(starts[0] <= starts[1]);

    delete_session(&pool, &s1).await;
    delete_session(&pool, &s2).await;
}

#[tokio::test]
async fn session_velocity_returns_count_and_duration() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;
    repo.increment_request_count(&sid).await.expect("req");

    let (count, duration) = repo.get_session_velocity(&sid).await.expect("velocity");
    assert_eq!(count, Some(1));
    assert!(duration.expect("duration") >= 0);

    // Missing session -> (None, None).
    let missing = unique_session_id();
    let (mc, md) = repo.get_session_velocity(&missing).await.expect("missing");
    assert_eq!(mc, None);
    assert_eq!(md, None);

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn update_behavioral_detection_and_mark_bot() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    repo.update_behavioral_detection(&sid, 80, true, Some("high_velocity"))
        .await
        .expect("update detection");

    let s = repo.find_by_id(&sid).await.expect("find").expect("present");
    assert_eq!(s.is_behavioral_bot, Some(true));
    assert_eq!(s.behavioral_bot_reason.as_deref(), Some("high_velocity"));

    repo.mark_as_behavioral_bot(&sid, "manual_flag")
        .await
        .expect("mark bot");

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn check_and_mark_behavioral_bot_threshold() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;
    for _ in 0..5 {
        repo.increment_request_count(&sid).await.expect("req");
    }

    // request_count (5) exceeds threshold 3 -> marked as behavioral bot.
    let flagged = repo
        .check_and_mark_behavioral_bot(&sid, 3)
        .await
        .expect("check");
    assert!(flagged);

    // A high threshold is not exceeded.
    let other = unique_session_id();
    seed_session(&repo, &other, &format!("fp-{}", Uuid::new_v4())).await;
    let not_flagged = repo
        .check_and_mark_behavioral_bot(&other, 1000)
        .await
        .expect("check2");
    assert!(!not_flagged);

    delete_session(&pool, &sid).await;
    delete_session(&pool, &other).await;
}

#[tokio::test]
async fn get_total_content_pages_is_non_negative() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let total = repo.get_total_content_pages().await.expect("total");
    assert!(total >= 0);
}
