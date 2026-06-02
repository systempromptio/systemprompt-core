//! DB-backed tests for the scheduler persistence layer.
//!
//! Each test acquires a real Postgres pool via the fixtures crate and
//! early-returns when `DATABASE_URL` is unset, so the suite still passes in
//! environments without a database. Every test owns uniquely-named rows
//! (`scheduled_jobs.job_name`) so concurrent shards never collide, and asserts
//! a concrete outcome: row present/absent, field values, or row counts.
//!
//! The `scheduled_jobs` table has no foreign-key dependency, so job CRUD is
//! exercised end-to-end through the public repository API. The analytics and
//! security repositories read/maintain `user_contexts` / `user_sessions`,
//! which have no fixtures seed helper; those tests therefore assert the query
//! executes and returns a well-formed (possibly empty) result set against the
//! freshly-migrated DB.

use systemprompt_scheduler::repository::{AnalyticsRepository, SecurityRepository};
use systemprompt_scheduler::{JobRepository, JobStatus, SchedulerRepository};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

// Returns None (skipping the test) when no integration DB is configured.
macro_rules! pool_or_skip {
    () => {{
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(pool) = fixture_db_pool(&url).await else {
            return;
        };
        pool
    }};
}

// Builds a name unique across processes (PID), tests (atomic counter), and
// reruns (nanosecond clock) without pulling in a uuid dependency.
fn unique_job_name(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}_{}_{}_{}", std::process::id(), seq, nanos)
}

mod scheduler_repository {
    use super::*;

    #[tokio::test]
    async fn new_succeeds_against_migrated_db() {
        let pool = pool_or_skip!();
        let _repo = SchedulerRepository::new(&pool).expect("composite repo should construct");
    }

    #[tokio::test]
    async fn upsert_then_find_returns_inserted_row() {
        let pool = pool_or_skip!();
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let name = unique_job_name("sched_upsert");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert should succeed");

        let found = repo.find_job(&name).await.expect("find should succeed");
        let job = found.expect("row should exist after upsert");
        assert_eq!(job.job_name, name);
        assert_eq!(job.schedule, "0 0 * * * *");
        assert!(job.enabled);
        assert_eq!(job.run_count, 0);
        assert!(job.last_run.is_none());
        assert!(job.last_status.is_none());
    }

    #[tokio::test]
    async fn find_missing_job_returns_none() {
        let pool = pool_or_skip!();
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let missing = unique_job_name("sched_absent");

        let found = repo.find_job(&missing).await.expect("find should succeed");
        assert!(found.is_none(), "a never-inserted job must not be found");
    }

    #[tokio::test]
    async fn upsert_conflict_updates_schedule_and_enabled() {
        let pool = pool_or_skip!();
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let name = unique_job_name("sched_conflict");

        repo.upsert_job(&name, "0 0 1 * * *", true)
            .await
            .expect("first upsert");
        repo.upsert_job(&name, "0 */5 * * * *", false)
            .await
            .expect("second upsert (conflict update)");

        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert_eq!(job.schedule, "0 */5 * * * *");
        assert!(!job.enabled, "conflict update should flip enabled to false");
    }

    #[tokio::test]
    async fn update_job_execution_persists_status_and_error() {
        let pool = pool_or_skip!();
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let name = unique_job_name("sched_exec");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.update_job_execution(&name, JobStatus::Failed, Some("boom"), None)
            .await
            .expect("update execution");

        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert_eq!(job.last_status.as_deref(), Some("failed"));
        assert_eq!(job.last_error.as_deref(), Some("boom"));
        assert!(job.last_run.is_some(), "last_run should be stamped");
    }

    #[tokio::test]
    async fn update_job_execution_success_clears_error() {
        let pool = pool_or_skip!();
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let name = unique_job_name("sched_success");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.update_job_execution(&name, JobStatus::Failed, Some("boom"), None)
            .await
            .expect("first update");
        repo.update_job_execution(&name, JobStatus::Success, None, None)
            .await
            .expect("second update");

        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert_eq!(job.last_status.as_deref(), Some("success"));
        assert!(job.last_error.is_none(), "success run should null the error");
    }

    #[tokio::test]
    async fn increment_run_count_accumulates() {
        let pool = pool_or_skip!();
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let name = unique_job_name("sched_runcount");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.increment_run_count(&name).await.expect("inc 1");
        repo.increment_run_count(&name).await.expect("inc 2");
        repo.increment_run_count(&name).await.expect("inc 3");

        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert_eq!(job.run_count, 3);
    }

    #[tokio::test]
    async fn list_enabled_jobs_includes_enabled_excludes_disabled() {
        let pool = pool_or_skip!();
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let enabled = unique_job_name("sched_list_on");
        let disabled = unique_job_name("sched_list_off");

        repo.upsert_job(&enabled, "0 0 * * * *", true)
            .await
            .expect("upsert enabled");
        repo.upsert_job(&disabled, "0 0 * * * *", false)
            .await
            .expect("upsert disabled");

        let rows = repo.list_enabled_jobs().await.expect("list enabled");
        let names: Vec<&str> = rows.iter().map(|j| j.job_name.as_str()).collect();
        assert!(
            names.contains(&enabled.as_str()),
            "enabled job must appear in list_enabled_jobs"
        );
        assert!(
            !names.contains(&disabled.as_str()),
            "disabled job must be excluded from list_enabled_jobs"
        );
    }

    #[tokio::test]
    async fn cleanup_empty_contexts_returns_rows_affected_count() {
        let pool = pool_or_skip!();
        let repo = SchedulerRepository::new(&pool).expect("repo");

        // No seeded contexts; the DELETE simply affects whatever stale empty
        // contexts exist (0 on a fresh DB). The call must succeed and return a
        // count.
        let affected = repo
            .cleanup_empty_contexts(1)
            .await
            .expect("cleanup should execute");
        let _ = affected;
    }
}

mod job_repository {
    use super::*;

    #[tokio::test]
    async fn new_succeeds() {
        let pool = pool_or_skip!();
        let _repo = JobRepository::new(&pool).expect("job repo should construct");
    }

    #[tokio::test]
    async fn set_enabled_toggles_flag() {
        let pool = pool_or_skip!();
        let repo = JobRepository::new(&pool).expect("repo");
        let name = unique_job_name("job_set_enabled");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.set_enabled(&name, false)
            .await
            .expect("set_enabled false");

        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert!(!job.enabled);

        repo.set_enabled(&name, true)
            .await
            .expect("set_enabled true");
        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert!(job.enabled);
    }

    #[tokio::test]
    async fn list_recent_runs_includes_executed_job() {
        let pool = pool_or_skip!();
        let repo = JobRepository::new(&pool).expect("repo");
        let name = unique_job_name("job_recent");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.update_job_execution(&name, JobStatus::Success, None, None)
            .await
            .expect("execute");

        let rows = repo.list_recent_runs(100).await.expect("list recent");
        let names: Vec<&str> = rows.iter().map(|j| j.job_name.as_str()).collect();
        assert!(
            names.contains(&name.as_str()),
            "a job with a stamped last_run must appear in list_recent_runs"
        );
    }

    #[tokio::test]
    async fn list_recent_runs_respects_limit() {
        let pool = pool_or_skip!();
        let repo = JobRepository::new(&pool).expect("repo");

        let rows = repo.list_recent_runs(2).await.expect("list recent");
        assert!(
            rows.len() <= 2,
            "list_recent_runs must honour the LIMIT, got {}",
            rows.len()
        );
    }

    #[tokio::test]
    async fn list_recent_runs_excludes_never_run_job() {
        let pool = pool_or_skip!();
        let repo = JobRepository::new(&pool).expect("repo");
        let name = unique_job_name("job_never_run");

        // Inserted but never executed: last_run stays NULL, so it must not
        // appear in the recent-runs view regardless of limit.
        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");

        let rows = repo
            .list_recent_runs(1000)
            .await
            .expect("list recent");
        let names: Vec<&str> = rows.iter().map(|j| j.job_name.as_str()).collect();
        assert!(
            !names.contains(&name.as_str()),
            "a job with NULL last_run must not appear in list_recent_runs"
        );
    }
}

mod analytics_repository {
    use super::*;

    #[tokio::test]
    async fn new_succeeds() {
        let pool = pool_or_skip!();
        let _repo = AnalyticsRepository::new(&pool).expect("analytics repo should construct");
    }

    #[tokio::test]
    async fn cleanup_empty_contexts_executes_for_various_windows() {
        let pool = pool_or_skip!();
        let repo = AnalyticsRepository::new(&pool).expect("repo");

        for hours in [0_i64, 1, 24, 168] {
            repo.cleanup_empty_contexts(hours)
                .await
                .expect("cleanup query should execute for any window");
        }
    }
}

mod security_repository {
    use super::*;

    #[tokio::test]
    async fn new_succeeds() {
        let pool = pool_or_skip!();
        let _repo = SecurityRepository::new(&pool).expect("security repo should construct");
    }

    #[tokio::test]
    async fn find_high_volume_ips_returns_well_formed_records() {
        let pool = pool_or_skip!();
        let repo = SecurityRepository::new(&pool).expect("repo");

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
        let pool = pool_or_skip!();
        let repo = SecurityRepository::new(&pool).expect("repo");

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
        let pool = pool_or_skip!();
        let repo = SecurityRepository::new(&pool).expect("repo");

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
        let pool = pool_or_skip!();
        let repo = SecurityRepository::new(&pool).expect("repo");

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
