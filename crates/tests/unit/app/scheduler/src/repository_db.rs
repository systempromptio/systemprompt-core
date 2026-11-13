//! DB-backed tests for the scheduler persistence layer.
//!
//! Each test acquires a real Postgres pool via the fixtures crate and
//! fails when `DATABASE_URL` is unset. Every test owns uniquely-named rows
//! (`scheduled_jobs.job_name`) so concurrent shards never collide, and asserts
//! a concrete outcome: row present/absent, field values, or row counts.
//!
//! The `scheduled_jobs` table has no foreign-key dependency, so job CRUD is
//! exercised end-to-end through the public repository API.

use systemprompt_identifiers::JobName;
use systemprompt_scheduler::{JobRepository, JobRunRecord, JobStatus, SchedulerRepository};
use systemprompt_test_fixtures::{test_db_pool, unique_instance};

// Returns None (skipping the test) when no integration DB is configured.

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

fn unique_job(prefix: &str) -> JobName {
    JobName::new(unique_job_name(prefix))
}

mod scheduler_repository {
    use super::*;

    #[tokio::test]
    async fn new_succeeds_against_migrated_db() {
        let pool = test_db_pool().await;
        let _repo = SchedulerRepository::new(&pool);
    }

    #[tokio::test]
    async fn upsert_then_find_returns_inserted_row() {
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        let name = unique_job("sched_upsert");

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
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        let missing = unique_job("sched_absent");

        let found = repo.find_job(&missing).await.expect("find should succeed");
        assert!(found.is_none(), "a never-inserted job must not be found");
    }

    #[tokio::test]
    async fn upsert_conflict_updates_schedule_and_enabled() {
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        let name = unique_job("sched_conflict");

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
        let test_node_instance = unique_instance();
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        let name = unique_job("sched_exec");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.update_job_execution(
            &name,
            JobRunRecord {
                status: JobStatus::Failed,
                error: Some("boom"),
                message: None,
                next_run: None,
                instance_id: &test_node_instance,
            },
        )
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
        let test_node_instance = unique_instance();
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        let name = unique_job("sched_success");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.update_job_execution(
            &name,
            JobRunRecord {
                status: JobStatus::Failed,
                error: Some("boom"),
                message: None,
                next_run: None,
                instance_id: &test_node_instance,
            },
        )
        .await
        .expect("first update");
        repo.update_job_execution(
            &name,
            JobRunRecord {
                status: JobStatus::Success,
                error: None,
                message: None,
                next_run: None,
                instance_id: &test_node_instance,
            },
        )
        .await
        .expect("second update");

        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert_eq!(job.last_status.as_deref(), Some("success"));
        assert!(
            job.last_error.is_none(),
            "success run should null the error"
        );
    }

    #[tokio::test]
    async fn each_recorded_run_advances_run_count() {
        let test_node_instance = unique_instance();
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        let name = unique_job("sched_runcount");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        for _ in 0..3 {
            repo.update_job_execution(
                &name,
                JobRunRecord {
                    status: JobStatus::Success,
                    error: None,
                    message: None,
                    next_run: None,
                    instance_id: &test_node_instance,
                },
            )
            .await
            .expect("record run");
        }

        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert_eq!(job.run_count, 3);
    }

    #[tokio::test]
    async fn list_enabled_jobs_includes_enabled_excludes_disabled() {
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        let enabled = unique_job("sched_list_on");
        let disabled = unique_job("sched_list_off");

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
}

mod job_repository {
    use super::*;

    #[tokio::test]
    async fn new_succeeds() {
        let pool = test_db_pool().await;
        let _repo = JobRepository::new(&pool);
    }

    #[tokio::test]
    async fn set_enabled_toggles_flag() {
        let pool = test_db_pool().await;
        let repo = JobRepository::new(&pool);
        let name = unique_job("job_set_enabled");

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
        let test_node_instance = unique_instance();
        let pool = test_db_pool().await;
        let repo = JobRepository::new(&pool);
        let name = unique_job("job_recent");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.update_job_execution(
            &name,
            JobRunRecord {
                status: JobStatus::Success,
                error: None,
                message: None,
                next_run: None,
                instance_id: &test_node_instance,
            },
        )
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
        let pool = test_db_pool().await;
        let repo = JobRepository::new(&pool);

        let rows = repo.list_recent_runs(2).await.expect("list recent");
        assert!(
            rows.len() <= 2,
            "list_recent_runs must honour the LIMIT, got {}",
            rows.len()
        );
    }

    #[tokio::test]
    async fn list_recent_runs_excludes_never_run_job() {
        let pool = test_db_pool().await;
        let repo = JobRepository::new(&pool);
        let name = unique_job("job_never_run");

        // Inserted but never executed: last_run stays NULL, so it must not
        // appear in the recent-runs view regardless of limit.
        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");

        let rows = repo.list_recent_runs(1000).await.expect("list recent");
        let names: Vec<&str> = rows.iter().map(|j| j.job_name.as_str()).collect();
        assert!(
            !names.contains(&name.as_str()),
            "a job with NULL last_run must not appear in list_recent_runs"
        );
    }
}

mod forget_retired_jobs {
    use super::*;

    #[tokio::test]
    async fn an_empty_inventory_forgets_nothing() {
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        let name = unique_job("sched_empty_inventory");
        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");

        let removed = repo
            .delete_jobs_not_in(&[])
            .await
            .expect("delete with an empty inventory");

        assert_eq!(
            removed, 0,
            "an empty inventory is not a list of retired jobs"
        );
        assert!(
            repo.find_job(&name).await.expect("find").is_some(),
            "the job row survives"
        );
    }

    #[tokio::test]
    async fn a_job_another_version_still_runs_is_kept() {
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        let other_version = unique_job("sched_other_version");
        repo.upsert_job(&other_version, "0 0 * * * *", true)
            .await
            .expect("upsert");

        repo.delete_jobs_not_in(&[unique_job("sched_this_build")])
            .await
            .expect("delete retired jobs");

        assert!(
            repo.find_job(&other_version).await.expect("find").is_some(),
            "a row touched inside the grace window belongs to a live version"
        );
    }
}
