//! DB-backed tests for the behavioural reads analytics composes
//! (`SessionSignalsRepository`) and the owner's behavioural queries and
//! detection writes reached through `SessionStore`. Sessions, analytics events
//! and engagement events are seeded with unique ids, then the windowed
//! aggregates and sequence/timestamp readers are asserted against known
//! expected values.

use chrono::{Duration, Utc};
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use uuid::Uuid;

use super::session_support::{
    base_params, delete_session, insert_analytics_event, insert_engagement_event, seed_session,
    unique_session_id,
};

#[tokio::test]
async fn count_sessions_by_fingerprint_counts_within_window() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repositories =
        systemprompt_test_fixtures::fixture_analytics_repositories(&pool).expect("repo");
    let store = &*repositories.session_store;

    let fp = format!("fp-{}", Uuid::new_v4());
    let s1 = unique_session_id();
    let s2 = unique_session_id();
    seed_session(store, &s1, &fp).await;
    seed_session(store, &s2, &fp).await;

    let count = store
        .count_sessions_by_fingerprint(&fp, 24)
        .await
        .expect("count");
    assert_eq!(count, 2);

    // A different fingerprint sees none.
    let other = store
        .count_sessions_by_fingerprint(&format!("fp-{}", Uuid::new_v4()), 24)
        .await
        .expect("count other");
    assert_eq!(other, 0);

    delete_session(&pool, &s1).await;
    delete_session(&pool, &s2).await;
}

#[tokio::test]
async fn endpoint_sequence_and_timestamps_ordered() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repositories =
        systemprompt_test_fixtures::fixture_analytics_repositories(&pool).expect("repo");
    let store = &*repositories.session_store;
    let signals = &repositories.session_signals;

    let sid = unique_session_id();
    seed_session(store, &sid, &format!("fp-{}", Uuid::new_v4())).await;

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

    let seq = signals.get_endpoint_sequence(&sid).await.expect("sequence");
    assert_eq!(seq, vec!["/a".to_owned(), "/b".to_owned()]);

    // Timestamps query returns all three events, ascending.
    let ts = signals
        .get_request_timestamps(&sid)
        .await
        .expect("timestamps");
    assert_eq!(ts.len(), 3);
    assert!(ts[0] <= ts[1] && ts[1] <= ts[2]);

    assert!(
        signals
            .has_analytics_events(&sid)
            .await
            .expect("has events")
    );

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn has_analytics_events_false_without_events() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repositories =
        systemprompt_test_fixtures::fixture_analytics_repositories(&pool).expect("repo");
    let store = &*repositories.session_store;
    let signals = &repositories.session_signals;

    let sid = unique_session_id();
    seed_session(store, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    assert!(!signals.has_analytics_events(&sid).await.expect("none"));
    let empty = signals
        .get_endpoint_sequence(&sid)
        .await
        .expect("empty seq");
    assert!(empty.is_empty());

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn session_for_behavioral_analysis_round_trip() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repositories =
        systemprompt_test_fixtures::fixture_analytics_repositories(&pool).expect("repo");
    let store = &*repositories.session_store;

    let sid = unique_session_id();
    let fp = format!("fp-{}", Uuid::new_v4());
    seed_session(store, &sid, &fp).await;
    store.increment_request_count(&sid).await.expect("req");

    let data = store
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
        store
            .get_session_for_behavioral_analysis(&missing)
            .await
            .expect("missing")
            .is_none()
    );

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn count_unique_ips_by_fingerprint() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repositories =
        systemprompt_test_fixtures::fixture_analytics_repositories(&pool).expect("repo");
    let store = &*repositories.session_store;

    let fp = format!("fp-{}", Uuid::new_v4());
    for ip in ["1.1.1.1", "2.2.2.2", "1.1.1.1"] {
        let sid = unique_session_id();
        let mut params = base_params(&sid, Some(&fp), Utc::now() + Duration::hours(1));
        params.ip_address = Some(ip);
        store
            .insert_session(&params)
            .await
            .expect("seed ip session");
    }

    let unique = store
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
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repositories =
        systemprompt_test_fixtures::fixture_analytics_repositories(&pool).expect("repo");
    let store = &*repositories.session_store;
    let signals = &repositories.session_signals;

    let fp = format!("fp-{}", Uuid::new_v4());
    let sid = unique_session_id();
    seed_session(store, &sid, &fp).await;
    insert_engagement_event(&pool, &sid).await;
    insert_engagement_event(&pool, &sid).await;

    let count = signals
        .count_engagement_events_by_fingerprint(&fp, 7)
        .await
        .expect("engagement count");
    assert_eq!(count, 2);

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn session_starts_by_fingerprint_ordered() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repositories =
        systemprompt_test_fixtures::fixture_analytics_repositories(&pool).expect("repo");
    let store = &*repositories.session_store;

    let fp = format!("fp-{}", Uuid::new_v4());
    let s1 = unique_session_id();
    let s2 = unique_session_id();
    seed_session(store, &s1, &fp).await;
    seed_session(store, &s2, &fp).await;

    let starts = store
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
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repositories =
        systemprompt_test_fixtures::fixture_analytics_repositories(&pool).expect("repo");
    let store = &*repositories.session_store;

    let sid = unique_session_id();
    seed_session(store, &sid, &format!("fp-{}", Uuid::new_v4())).await;
    store.increment_request_count(&sid).await.expect("req");

    let (count, duration) = store.get_session_velocity(&sid).await.expect("velocity");
    assert_eq!(count, Some(1));
    assert!(duration.expect("duration") >= 0);

    // Missing session -> (None, None).
    let missing = unique_session_id();
    let (mc, md) = store.get_session_velocity(&missing).await.expect("missing");
    assert_eq!(mc, None);
    assert_eq!(md, None);

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn update_behavioral_detection_and_mark_bot() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repositories =
        systemprompt_test_fixtures::fixture_analytics_repositories(&pool).expect("repo");
    let store = &*repositories.session_store;

    let sid = unique_session_id();
    seed_session(store, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    store
        .update_behavioral_detection(&sid, 80, true, Some("high_velocity"))
        .await
        .expect("update detection");

    let s = store
        .find_by_id(&sid)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(s.is_behavioral_bot, Some(true));
    assert_eq!(s.behavioral_bot_reason.as_deref(), Some("high_velocity"));

    store
        .mark_as_behavioral_bot(&sid, "manual_flag")
        .await
        .expect("mark bot");

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn check_and_mark_behavioral_bot_threshold() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repositories =
        systemprompt_test_fixtures::fixture_analytics_repositories(&pool).expect("repo");
    let store = &*repositories.session_store;

    let sid = unique_session_id();
    seed_session(store, &sid, &format!("fp-{}", Uuid::new_v4())).await;
    for _ in 0..5 {
        store.increment_request_count(&sid).await.expect("req");
    }

    // request_count (5) exceeds threshold 3 -> marked as behavioral bot.
    let flagged = store
        .check_and_mark_behavioral_bot(&sid, 3)
        .await
        .expect("check");
    assert!(flagged);

    // A high threshold is not exceeded.
    let other = unique_session_id();
    seed_session(store, &other, &format!("fp-{}", Uuid::new_v4())).await;
    let not_flagged = store
        .check_and_mark_behavioral_bot(&other, 1000)
        .await
        .expect("check2");
    assert!(!not_flagged);

    delete_session(&pool, &sid).await;
    delete_session(&pool, &other).await;
}

#[tokio::test]
async fn get_total_content_pages_is_non_negative() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repositories =
        systemprompt_test_fixtures::fixture_analytics_repositories(&pool).expect("repo");
    let signals = &repositories.session_signals;

    let total = signals.get_total_content_pages().await.expect("total");
    assert!(total >= 0);
}
