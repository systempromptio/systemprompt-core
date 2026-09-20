use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use systemprompt_models::SchedulerConfig;
use systemprompt_scheduler::{JobStatus, SchedulerRepository, SchedulerService};
use systemprompt_test_fixtures::{DisposableDb, fixture_app_context};
use tracing_subscriber::layer::SubscriberExt;

use crate::test_jobs::{
    CANCELLABLE_CLUSTER_JOB, CANCELLABLE_CLUSTER_JOB_RUNS, cancellable_cluster_job_started,
};

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("capture").extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test]
async fn cancelling_a_cluster_job_releases_its_session_lock_for_a_later_dispatch() {
    let diagnostics = Capture::default();
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_target(false)
            .with_writer(diagnostics.clone()),
    );
    let _subscriber = tracing::subscriber::set_default(subscriber);
    let database = DisposableDb::installed("scheduler_cancelled_cluster_job")
        .await
        .unwrap();
    let pool = database.pool().await.unwrap();
    let context = fixture_app_context(&pool, database.url()).unwrap();
    let repository = SchedulerRepository::new(&pool).unwrap();
    repository
        .upsert_job(CANCELLABLE_CLUSTER_JOB, "", true)
        .await
        .unwrap();
    let service = Arc::new(
        SchedulerService::new(
            SchedulerConfig {
                enabled: true,
                jobs: Vec::new(),
                bootstrap_jobs: vec![CANCELLABLE_CLUSTER_JOB.to_owned()],
                distributed_lock: true,
            },
            Arc::clone(&pool),
            context,
        )
        .unwrap(),
    );

    CANCELLABLE_CLUSTER_JOB_RUNS.store(0, Ordering::SeqCst);
    let started = cancellable_cluster_job_started().notified();
    let first_service = Arc::clone(&service);
    let first = tokio::spawn(async move { first_service.run_bootstrap_jobs(None).await });
    tokio::time::timeout(Duration::from_secs(2), started)
        .await
        .expect("first job acquired its cluster claim and entered execute");
    first.abort();
    assert!(
        first
            .await
            .expect_err("dispatch was cancelled")
            .is_cancelled()
    );

    let cancelled = repository
        .find_job(CANCELLABLE_CLUSTER_JOB)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        cancelled.last_status.as_deref(),
        Some(JobStatus::Running.as_str())
    );
    assert_eq!(cancelled.run_count, 1);
    let events: Vec<serde_json::Value> =
        String::from_utf8(diagnostics.0.lock().expect("capture").clone())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    let dropped = events
        .into_iter()
        .find(|event| {
            event["fields"]["message"]
                == "JobLockGuard dropped without explicit release; closing its session"
        })
        .expect("cancelled claim warning");
    assert_eq!(dropped["fields"]["job_name"], CANCELLABLE_CLUSTER_JOB);

    let deadline = Instant::now() + Duration::from_secs(5);
    while CANCELLABLE_CLUSTER_JOB_RUNS.load(Ordering::SeqCst) != 2 {
        assert!(
            Instant::now() < deadline,
            "detached advisory-lock session did not close"
        );
        service
            .run_bootstrap_jobs(None)
            .await
            .expect("bounded recovery dispatch");
        if CANCELLABLE_CLUSTER_JOB_RUNS.load(Ordering::SeqCst) != 2 {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    let recovered = repository
        .find_job(CANCELLABLE_CLUSTER_JOB)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        recovered.last_status.as_deref(),
        Some(JobStatus::Success.as_str())
    );
    assert_eq!(
        recovered.run_count, 2,
        "lock-skipped retries must not count as runs"
    );

    pool.write_pool_arc().unwrap().close().await;
    drop(repository);
    drop(service);
    drop(pool);
    database.drop_now().await;
}
