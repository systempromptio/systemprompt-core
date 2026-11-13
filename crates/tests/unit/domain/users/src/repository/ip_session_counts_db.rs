//! DB-backed tests for `SessionRepository::ip_session_counts`, the per-IP
//! session counts the malicious-IP blacklist classifies. The queries run
//! against a freshly migrated database and must return well-formed (possibly
//! empty) results.

use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::session_store::IpSessionQuery;
use systemprompt_users::SessionRepository;

#[tokio::test]
async fn human_counts_respect_the_threshold() {
    let pool = test_db_pool().await;
    let repo = SessionRepository::new(&pool);

    let records = repo
        .ip_session_counts(IpSessionQuery::HumanAtLeast(i64::MAX))
        .await
        .expect("query should execute");
    assert!(records.is_empty());

    let records = repo
        .ip_session_counts(IpSessionQuery::HumanAtLeast(1))
        .await
        .expect("query should execute");
    for rec in &records {
        assert!(!rec.ip_address.is_empty());
        assert!(rec.session_count >= 1);
        assert!(rec.country.is_none());
    }
}

#[tokio::test]
async fn scanner_counts_execute() {
    let pool = test_db_pool().await;
    let records = SessionRepository::new(&pool)
        .ip_session_counts(IpSessionQuery::ScannerAtLeast(1))
        .await
        .expect("scanner query should execute");
    for rec in &records {
        assert!(rec.session_count >= 1);
        assert!(rec.country.is_none());
    }
}

#[tokio::test]
async fn all_counts_execute() {
    let pool = test_db_pool().await;
    let records = SessionRepository::new(&pool)
        .ip_session_counts(IpSessionQuery::All)
        .await
        .expect("recent query should execute");
    for rec in &records {
        assert!(rec.session_count >= 1);
        assert!(rec.country.is_none());
    }
}

#[tokio::test]
async fn country_counts_carry_the_country() {
    let pool = test_db_pool().await;
    let records = SessionRepository::new(&pool)
        .ip_session_counts(IpSessionQuery::CountryAtLeast(1))
        .await
        .expect("country query should execute");
    for rec in &records {
        assert!(rec.country.is_some());
        assert!(rec.session_count >= 1);
    }
}
