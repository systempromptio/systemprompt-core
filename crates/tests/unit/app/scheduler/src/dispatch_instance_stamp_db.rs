//! DB-backed tests for the bookkeeping `scheduling/dispatch.rs` performs on the
//! unhappy paths: a job that returns a failure and a job that panics must both
//! still stamp the running replica's `instance_id` on the `scheduled_jobs` row,
//! and a config-level `scope: node` override must beat a peer-held cluster
//! advisory lock.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use systemprompt_database::DbPool;
use systemprompt_models::services::scheduler::JobScope;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{
    JobConfig, JobStatus, SchedulerConfig, SchedulerRepository, SchedulerService,
};
use systemprompt_test_fixtures::{
    fixture_app_context_with_config, fixture_config, fixture_database_url, fixture_db_pool,
};

use crate::test_jobs::{STAMP_FAIL_JOB, STAMP_FAIL_JOB_RUNS, STAMP_PANIC_JOB};

// Why: these tests own one `scheduled_jobs` row per job name and reset its
// dedupe columns, so they must not interleave within the process.
static SERIALIZE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn pool_and_url() -> (DbPool, String) {
    let url = fixture_database_url().expect("DATABASE_URL must be set");
    let pool = fixture_db_pool(&url).await.expect("db pool");
    (pool, url)
}

fn context_for_instance(pool: &DbPool, url: &str, instance_id: &str) -> Arc<AppContext> {
    let mut config = fixture_config(url);
    config.instance_id = instance_id.to_owned();
    fixture_app_context_with_config(pool, config).expect("fixture AppContext")
}

async fn seed_row(pool: &DbPool, job_name: &str) -> SchedulerRepository {
    let repo = SchedulerRepository::new(pool).expect("repo");
    repo.upsert_job(job_name, "", true)
        .await
        .expect("seed scheduled_jobs row");
    let pg = pool.write_pool_arc().expect("write pool");
    sqlx::query!(
        "UPDATE scheduled_jobs SET last_run = NULL, last_instance_id = NULL, last_error = NULL \
         WHERE job_name = $1",
        job_name
    )
    .execute(&*pg)
    .await
    .expect("reset dedupe columns");
    repo
}

async fn dispatch(
    pool: &DbPool,
    url: &str,
    instance_id: &str,
    job_name: &str,
    jobs: Vec<JobConfig>,
    distributed_lock: bool,
) {
    let app_ctx = context_for_instance(pool, url, instance_id);
    let config = SchedulerConfig {
        enabled: true,
        jobs,
        bootstrap_jobs: vec![job_name.to_owned()],
        distributed_lock,
    };
    let svc =
        SchedulerService::new(config, Arc::clone(pool), app_ctx).expect("SchedulerService::new");
    svc.run_bootstrap_jobs(None)
        .await
        .expect("bootstrap dispatch must not abort");
}

#[tokio::test]
async fn failing_job_records_the_error_and_stamps_this_replica() {
    let (pool, url) = pool_and_url().await;
    let _guard = SERIALIZE.lock().await;
    let repo = seed_row(&pool, STAMP_FAIL_JOB).await;

    dispatch(
        &pool,
        &url,
        "stamp-node-fail",
        STAMP_FAIL_JOB,
        Vec::new(),
        false,
    )
    .await;

    let row = repo
        .find_job(STAMP_FAIL_JOB)
        .await
        .expect("find_job")
        .expect("seeded row");
    assert_eq!(row.last_status.as_deref(), Some(JobStatus::Failed.as_str()));
    assert_eq!(
        row.last_error.as_deref(),
        Some("stamp job failed on purpose")
    );
    assert_eq!(
        row.last_instance_id.as_deref(),
        Some("stamp-node-fail"),
        "a failed run must still record which replica ran it"
    );
}

#[tokio::test]
async fn panicking_job_stamps_this_replica() {
    let (pool, url) = pool_and_url().await;
    let _guard = SERIALIZE.lock().await;
    let repo = seed_row(&pool, STAMP_PANIC_JOB).await;

    dispatch(
        &pool,
        &url,
        "stamp-node-panic",
        STAMP_PANIC_JOB,
        Vec::new(),
        false,
    )
    .await;

    let row = repo
        .find_job(STAMP_PANIC_JOB)
        .await
        .expect("find_job")
        .expect("seeded row");
    assert_eq!(row.last_status.as_deref(), Some(JobStatus::Failed.as_str()));
    assert!(
        row.last_error
            .as_deref()
            .is_some_and(|e| e.contains("stamp job panic payload")),
        "the panic payload must reach the row, got {:?}",
        row.last_error
    );
    assert_eq!(
        row.last_instance_id.as_deref(),
        Some("stamp-node-panic"),
        "a panicked run must still record which replica ran it"
    );
}

#[tokio::test]
async fn config_scope_node_overrides_a_cluster_default_job_under_a_peer_lock() {
    let (pool, url) = pool_and_url().await;
    let _guard = SERIALIZE.lock().await;
    seed_row(&pool, STAMP_FAIL_JOB).await;

    let pg = pool.write_pool_arc().expect("write pool");
    let mut peer = pg.acquire().await.expect("peer connection");
    let key: i64 = sqlx::query_scalar!(r#"SELECT hashtext($1)::bigint AS "key!""#, STAMP_FAIL_JOB)
        .fetch_one(peer.as_mut())
        .await
        .expect("hash job name");
    let acquired: Option<bool> =
        sqlx::query_scalar!(r#"SELECT pg_try_advisory_lock($1) AS "acquired""#, key)
            .fetch_one(peer.as_mut())
            .await
            .expect("peer lock");
    assert_eq!(
        acquired,
        Some(true),
        "peer must win the advisory lock first"
    );

    let before = STAMP_FAIL_JOB_RUNS.load(Ordering::SeqCst);
    let jobs = vec![JobConfig::new(STAMP_FAIL_JOB).with_scope(JobScope::Node)];
    dispatch(&pool, &url, "stamp-node-x", STAMP_FAIL_JOB, jobs, true).await;

    sqlx::query_scalar!("SELECT pg_advisory_unlock($1)", key)
        .fetch_one(peer.as_mut())
        .await
        .expect("peer unlock");

    assert_eq!(
        STAMP_FAIL_JOB_RUNS.load(Ordering::SeqCst) - before,
        1,
        "a config-level node scope must not take the cluster advisory lock"
    );
}
