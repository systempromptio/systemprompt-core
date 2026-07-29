//! DB-backed tests for the historical GeoIP backfill. Without a reader the
//! sweep must be a no-op: every candidate row is skipped, so a later run with
//! a real database can still enrich it.

use chrono::{Duration, Utc};
use systemprompt_analytics::SessionRepository;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

use super::session_support::{base_params, delete_session, unique_session_id};

#[tokio::test]
async fn backfill_without_a_reader_updates_nothing_and_is_idempotent() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let sid = unique_session_id();
    let fp = format!("fp-{}", Uuid::new_v4());
    let mut params = base_params(&sid, Some(&fp), Utc::now() + Duration::hours(1));
    params.ip_address = Some("8.8.8.8");
    repo.create_session(&params).await.expect("seed session");

    let missing = repo
        .count_sessions_missing_geo()
        .await
        .expect("count candidates");
    assert!(
        missing >= 1,
        "a session with an IP and no country must be counted as a backfill candidate"
    );

    let updated = repo
        .backfill_session_geo(None, 100)
        .await
        .expect("backfill runs");
    assert_eq!(
        updated, 0,
        "with no GeoIP reader every candidate must be skipped"
    );

    let row = repo.find_by_id(&sid).await.expect("find").expect("present");
    assert!(
        row.country.is_none(),
        "a skipped row must keep its NULL country so a later run can enrich it"
    );

    let second = repo
        .backfill_session_geo(None, 100)
        .await
        .expect("second backfill runs");
    assert_eq!(second, 0, "the sweep must be idempotent");
    assert!(
        repo.count_sessions_missing_geo()
            .await
            .expect("recount candidates")
            >= 1,
        "a reader-less run must not consume the candidate"
    );

    delete_session(&pool, &sid).await;
}
