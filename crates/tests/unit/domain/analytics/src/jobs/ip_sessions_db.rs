//! DB-backed tests for [`IpSessionRepository`], the per-IP session counts the
//! malicious-IP blacklist classifies. The queries run against a freshly
//! migrated database and must return well-formed (possibly empty) results.

use systemprompt_analytics::repository::IpSessionRepository;
use systemprompt_test_fixtures::test_db_pool;

mod ip_session_repository {
    use super::*;

    #[tokio::test]
    async fn new_succeeds() {
        let pool = test_db_pool().await;
        let _repo = IpSessionRepository::new(&pool);
    }

    #[tokio::test]
    async fn find_high_volume_ips_returns_well_formed_records() {
        let pool = test_db_pool().await;
        let repo = IpSessionRepository::new(&pool);

        // A very high threshold guarantees an empty result on any realistic
        // DB, but the aggregation + filter_map mapping path still executes.
        let records = repo
            .find_high_volume_ips(i64::MAX)
            .await
            .expect("query should execute");
        assert!(records.is_empty());

        // A threshold of 1 may surface real rows; every returned record must
        // carry the ip_address the mapping populates.
        let records = repo
            .find_high_volume_ips(1)
            .await
            .expect("query should execute");
        for rec in &records {
            assert!(rec.ip_address.is_some());
            assert!(rec.session_count >= 1);
            assert!(rec.country.is_none());
        }
    }

    #[tokio::test]
    async fn find_scanner_ips_executes() {
        let pool = test_db_pool().await;
        let repo = IpSessionRepository::new(&pool);

        let records = repo
            .find_scanner_ips(1)
            .await
            .expect("scanner query should execute");
        for rec in &records {
            assert!(rec.ip_address.is_some());
            assert!(rec.country.is_none());
        }
    }

    #[tokio::test]
    async fn find_recent_ips_executes() {
        let pool = test_db_pool().await;
        let repo = IpSessionRepository::new(&pool);

        let records = repo
            .find_recent_ips()
            .await
            .expect("recent-ips query should execute");
        for rec in &records {
            assert!(rec.ip_address.is_some());
        }
    }

    #[tokio::test]
    async fn find_high_risk_country_ips_populates_country() {
        let pool = test_db_pool().await;
        let repo = IpSessionRepository::new(&pool);

        let records = repo
            .find_high_risk_country_ips(i64::MAX)
            .await
            .expect("country query should execute");
        assert!(records.is_empty());

        let records = repo
            .find_high_risk_country_ips(1)
            .await
            .expect("country query should execute");
        for rec in &records {
            assert!(rec.ip_address.is_some());
            // This query selects country, so any returned record carries it.
            assert!(rec.country.is_some());
        }
    }
}

