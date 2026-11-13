//! DB-backed tests for the IP-ban detectors' `execute` bodies on an empty
//! database, and their failure on a dead pool.

use systemprompt_analytics::jobs::{BehavioralAnalysisJob, MaliciousIpBlacklistJob};
use systemprompt_test_fixtures::{test_database_url, test_db_pool};
use systemprompt_traits::Job;

use super::{job_ctx as make_test_ctx, job_ctx_over};

mod behavioral_analysis_db {
    use super::*;

    #[tokio::test]
    async fn execute_succeeds_against_empty_db() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let ctx = make_test_ctx(&pool, &url);
        let job = BehavioralAnalysisJob;

        let result = job
            .execute(&ctx)
            .await
            .expect("BehavioralAnalysisJob::execute must not error on an empty DB");

        assert!(
            result.success,
            "BehavioralAnalysisJob must report success when no fingerprints require action"
        );
    }

    #[tokio::test]
    async fn execute_returns_valid_duration() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let ctx = make_test_ctx(&pool, &url);
        let job = BehavioralAnalysisJob;

        let result = job.execute(&ctx).await.expect("execute must not error");

        let _ = result.duration_ms;
    }

    #[tokio::test]
    async fn execute_has_stats_fields() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let ctx = make_test_ctx(&pool, &url);
        let job = BehavioralAnalysisJob;

        let result = job.execute(&ctx).await.expect("execute must not error");

        assert_eq!(
            result.items_failed.unwrap_or(0),
            0,
            "BehavioralAnalysisJob must report 0 failures on an empty DB"
        );
    }
}

mod malicious_ip_blacklist_db {
    use super::*;

    #[tokio::test]
    async fn execute_succeeds_against_empty_db() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let ctx = make_test_ctx(&pool, &url);
        let job = MaliciousIpBlacklistJob;

        let result = job
            .execute(&ctx)
            .await
            .expect("MaliciousIpBlacklistJob::execute must not error on an empty DB");

        assert!(
            result.success,
            "MaliciousIpBlacklistJob must report success when no candidates are found"
        );
    }

    #[tokio::test]
    async fn execute_reports_zero_banned_on_empty_db() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let ctx = make_test_ctx(&pool, &url);
        let job = MaliciousIpBlacklistJob;

        let result = job.execute(&ctx).await.expect("execute must not error");

        assert_eq!(
            result.items_processed.unwrap_or(0),
            0,
            "no IPs should be banned against an empty DB"
        );
    }

    #[tokio::test]
    async fn execute_is_idempotent() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let job = MaliciousIpBlacklistJob;

        for _ in 0..2 {
            let ctx = make_test_ctx(&pool, &url);
            job.execute(&ctx)
                .await
                .expect("repeated MaliciousIpBlacklistJob executions must all succeed");
        }
    }

    async fn seed_scanner_sessions(pool: &systemprompt_database::DbPool, ip: &str, count: i32) {
        let pg = pool.pool();
        for i in 0..count {
            sqlx::query(
                "INSERT INTO user_sessions (session_id, ip_address, is_scanner, started_at, \
                 last_activity_at) VALUES ($1, $2, true, NOW(), NOW())",
            )
            .bind(format!("scanner_enforce_{ip}_{i}"))
            .bind(ip)
            .execute(&*pg)
            .await
            .expect("seed scanner session");
        }
    }

    async fn cleanup_seed(pool: &systemprompt_database::DbPool, ip: &str) {
        let pg = pool.pool();
        sqlx::query("DELETE FROM banned_ips WHERE ip_address = $1")
            .bind(ip)
            .execute(&*pg)
            .await
            .expect("cleanup banned_ips");
        sqlx::query("DELETE FROM user_sessions WHERE ip_address = $1")
            .bind(ip)
            .execute(&*pg)
            .await
            .expect("cleanup sessions");
    }

    async fn is_ip_banned(pool: &systemprompt_database::DbPool, ip: &str) -> bool {
        let pg = pool.pool();
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM banned_ips WHERE ip_address = $1")
                .bind(ip)
                .fetch_one(&*pg)
                .await
                .expect("count banned_ips");
        count > 0
    }

    // One sequential test rather than two: an enforce=true execution bans
    // every qualifying candidate in the shared fixture DB, so a concurrent
    // no-enforce test's seeded IP would be banned by the other test's run.
    #[tokio::test]
    async fn enforce_flag_gates_banning_of_qualifying_candidates() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let ip = format!("198.51.100.{}", std::process::id() % 200);
        cleanup_seed(&pool, &ip).await;
        seed_scanner_sessions(&pool, &ip, 3).await;

        let observe_ctx = make_test_ctx(&pool, &url);
        MaliciousIpBlacklistJob
            .execute(&observe_ctx)
            .await
            .expect("execute must not error");
        assert!(
            !is_ip_banned(&pool, &ip).await,
            "enforce defaults to false, so a qualifying scanner IP must not be banned"
        );

        let enforce_ctx = make_test_ctx(&pool, &url).with_enforce(true);
        MaliciousIpBlacklistJob
            .execute(&enforce_ctx)
            .await
            .expect("execute must not error");
        assert!(
            is_ip_banned(&pool, &ip).await,
            "with enforce=true the qualifying scanner IP must be banned"
        );

        cleanup_seed(&pool, &ip).await;
    }
}

mod closed_pool_error_propagation {
    use super::*;
    use systemprompt_test_fixtures::closed_db_pool;

    #[tokio::test]
    async fn behavioral_analysis_propagates_a_dead_pool_error() {
        let url = test_database_url();
        let real_pool = test_db_pool().await;
        let closed = closed_db_pool().await;
        let ctx = job_ctx_over(&real_pool, &closed, &url);

        BehavioralAnalysisJob
            .execute(&ctx)
            .await
            .expect_err("a dead pool must surface as an execute error, not success");
    }

    #[tokio::test]
    async fn malicious_ip_blacklist_propagates_a_dead_pool_error() {
        let url = test_database_url();
        let real_pool = test_db_pool().await;
        let closed = closed_db_pool().await;
        let ctx = job_ctx_over(&real_pool, &closed, &url);

        MaliciousIpBlacklistJob
            .execute(&ctx)
            .await
            .expect_err("a dead pool must surface as an execute error, not success");
    }
}
