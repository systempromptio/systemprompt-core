//! Tests for `JobExecutionService`: parameter parsing, selection resolution,
//! on-demand execution, and run recording against the fixture DB. DB-backed
//! tests early-return when `DATABASE_URL` is unset.

use std::collections::HashMap;

use systemprompt_extension::ExtensionRegistry;
use systemprompt_scheduler::{
    JobExecutionService, JobRepository, JobSelection, SchedulerError, parse_job_parameters,
};
use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url, fixture_db_pool};

macro_rules! service_or_skip {
    () => {{
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(pool) = fixture_db_pool(&url).await else {
            return;
        };
        let app_ctx = fixture_app_context(&pool, &url)
            .expect("fixture AppContext must build against a migrated DB");
        (
            JobExecutionService::new(app_ctx, ExtensionRegistry::new()),
            pool,
        )
    }};
}

mod parameter_parsing {
    use super::*;

    #[test]
    fn parses_key_value_pairs() {
        let params = vec!["alpha=1".to_owned(), "beta=two".to_owned()];
        let parsed = parse_job_parameters(&params).expect("valid KEY=VALUE pairs must parse");

        assert_eq!(parsed.get("alpha").map(String::as_str), Some("1"));
        assert_eq!(parsed.get("beta").map(String::as_str), Some("two"));
    }

    #[test]
    fn keeps_equals_signs_in_values() {
        let params = vec!["query=a=b=c".to_owned()];
        let parsed = parse_job_parameters(&params).expect("value may contain '='");

        assert_eq!(parsed.get("query").map(String::as_str), Some("a=b=c"));
    }

    #[test]
    fn rejects_entry_without_equals() {
        let err = parse_job_parameters(&["broken".to_owned()])
            .expect_err("entry without '=' must be rejected");

        assert!(matches!(
            &err,
            SchedulerError::InvalidJobParameter { parameter } if parameter == "broken"
        ));
        assert_eq!(
            err.to_string(),
            "Invalid parameter format 'broken'. Use KEY=VALUE format."
        );
    }

    #[test]
    fn empty_input_yields_empty_map() {
        let parsed = parse_job_parameters(&[]).expect("empty input must parse");
        assert!(parsed.is_empty());
    }
}

mod selection_resolution {
    use super::*;

    #[tokio::test]
    async fn empty_names_selection_is_rejected() {
        let (service, _pool) = service_or_skip!();

        let err = service
            .resolve_job_names(&JobSelection::Names(vec![]))
            .expect_err("empty explicit selection must be rejected");

        assert_eq!(
            err.to_string(),
            "Specify job name(s), use --all, or use --tag <tag> to run jobs"
        );
    }

    #[tokio::test]
    async fn unknown_tag_is_rejected() {
        let (service, _pool) = service_or_skip!();

        let err = service
            .resolve_job_names(&JobSelection::Tag("no-such-tag".to_owned()))
            .expect_err("a tag matching no jobs must be rejected");

        assert_eq!(err.to_string(), "No jobs found with tag 'no-such-tag'");
    }

    #[tokio::test]
    async fn explicit_names_pass_through_unvalidated() {
        let (service, _pool) = service_or_skip!();

        let names = service
            .resolve_job_names(&JobSelection::Names(vec!["anything".to_owned()]))
            .expect("explicit names resolve as given");

        assert_eq!(names, vec!["anything".to_owned()]);
    }

    #[tokio::test]
    async fn all_selection_includes_inventory_jobs() {
        let (service, _pool) = service_or_skip!();

        let names = service
            .resolve_job_names(&JobSelection::All)
            .expect("All selection must resolve");

        assert!(
            names.iter().any(|n| n == "cleanup_inactive_sessions"),
            "inventory-registered scheduler jobs must appear in the All selection: {names:?}"
        );
    }
}

mod execution {
    use super::*;

    #[tokio::test]
    async fn unknown_job_reports_failure_without_erroring() {
        let (service, _pool) = service_or_skip!();

        let report = service.run_job("no_such_job", &HashMap::new()).await;

        assert!(!report.success);
        assert_eq!(report.job_name, "no_such_job");
        assert_eq!(
            report.message.as_deref(),
            Some("Job 'no_such_job' not found")
        );
    }

    #[tokio::test]
    async fn batch_counts_successes_and_failures() {
        let (service, _pool) = service_or_skip!();

        let batch = service
            .run_jobs(
                &JobSelection::Names(vec![
                    "cleanup_inactive_sessions".to_owned(),
                    "no_such_job".to_owned(),
                ]),
                &HashMap::new(),
            )
            .await
            .expect("explicit-name batch must run");

        assert_eq!(batch.runs.len(), 2);
        assert_eq!(batch.succeeded, 1);
        assert_eq!(batch.failed, 1);
        assert!(batch.runs[0].success);
        assert!(!batch.runs[1].success);
    }

    #[tokio::test]
    async fn run_is_recorded_on_the_scheduled_jobs_row() {
        let (service, pool) = service_or_skip!();

        let repo = JobRepository::new(&pool).expect("construct JobRepository");
        repo.upsert_job("cleanup_inactive_sessions", "0 0 * * * *", true)
            .await
            .expect("seed scheduled_jobs row");

        let report = service
            .run_job("cleanup_inactive_sessions", &HashMap::new())
            .await;
        assert!(report.success, "job must succeed on an empty DB");

        let row = repo
            .find_job("cleanup_inactive_sessions")
            .await
            .expect("find_job must succeed")
            .expect("seeded row must exist");

        assert_eq!(row.last_status.as_deref(), Some("success"));
        assert!(row.last_run.is_some());
        assert!(row.run_count >= 1);
    }
}

mod manual_run_recording_arms {
    use super::*;
    use crate::test_jobs::FAILING_JOB;
    use systemprompt_scheduler::JobStatus;

    #[tokio::test]
    async fn debug_output_names_the_service() {
        let (service, _pool) = service_or_skip!();
        assert!(format!("{service:?}").contains("JobExecutionService"));
    }

    #[tokio::test]
    async fn failed_manual_run_records_failed_status_and_message() {
        let (service, pool) = service_or_skip!();

        let repo = JobRepository::new(&pool).expect("construct JobRepository");
        repo.upsert_job(FAILING_JOB, "", true)
            .await
            .expect("seed scheduled_jobs row");

        let report = service.run_job(FAILING_JOB, &HashMap::new()).await;
        assert!(!report.success);
        assert_eq!(report.message.as_deref(), Some("deliberate test failure"));

        let row = repo
            .find_job(FAILING_JOB)
            .await
            .expect("find_job")
            .expect("seeded row must exist");
        assert_eq!(row.last_status.as_deref(), Some(JobStatus::Failed.as_str()));
        assert_eq!(row.last_error.as_deref(), Some("deliberate test failure"));
    }

    #[tokio::test]
    async fn manual_run_without_scheduled_jobs_row_is_not_recorded() {
        let (service, pool) = service_or_skip!();

        let pg = pool.write_pool_arc().expect("write pool");
        sqlx::query!(
            "DELETE FROM scheduled_jobs WHERE job_name = $1",
            FAILING_JOB
        )
        .execute(&*pg)
        .await
        .expect("clear test-job row");

        let report = service.run_job(FAILING_JOB, &HashMap::new()).await;
        assert!(!report.success, "the job itself still runs and fails");

        let repo = JobRepository::new(&pool).expect("construct JobRepository");
        assert!(
            repo.find_job(FAILING_JOB)
                .await
                .expect("find_job")
                .is_none(),
            "run recording must not create a scheduled_jobs row"
        );
    }
}

mod dead_pool_recording {
    use super::*;
    use crate::test_jobs::FAILING_JOB;
    use systemprompt_test_fixtures::closed_db_pool;

    #[tokio::test]
    async fn run_survives_an_unreachable_database() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(real_pool) = fixture_db_pool(&url).await else {
            return;
        };
        let _ = real_pool;
        let closed = closed_db_pool().await;
        let Ok(app_ctx) = fixture_app_context(&closed, &url) else {
            return;
        };
        let service = JobExecutionService::new(app_ctx, ExtensionRegistry::new());

        // The job body and every recording query hit the closed pool; the run
        // still yields a report instead of propagating the DB failure.
        let report = service.run_job(FAILING_JOB, &HashMap::new()).await;
        assert!(!report.success);
        assert_eq!(report.message.as_deref(), Some("deliberate test failure"));
    }
}
