//! A manual run (`infra jobs run`) claims a cluster job through the same
//! advisory lock the cron loop takes. A peer replica holding it makes the run
//! fail as contended without touching the job's row; a bounded wait lets the
//! run proceed once the peer lets go. Node jobs, and every job when the
//! distributed lock is off, run unclaimed. These tests take advisory locks on
//! shared job names and join the serialized `scheduler-jobs-db` nextest group.

use std::collections::HashMap;
use std::time::Duration;

use sqlx::Postgres;
use sqlx::pool::PoolConnection;
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::JobName;
use systemprompt_scheduler::{JobExecutionService, JobSelection, SchedulerConfig};
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};

use crate::test_jobs::{NODE_JOB, WORKING_JOB};

async fn hold_peer_lock(pool: &DbPool, job_name: &str) -> PoolConnection<Postgres> {
    let mut peer = pool.write_pool().acquire().await.expect("peer connection");
    let key: i64 = sqlx::query_scalar!(r#"SELECT hashtext($1)::bigint AS "key!""#, job_name)
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
    peer
}

async fn service(config: SchedulerConfig) -> (JobExecutionService, DbPool) {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let app_ctx = test_app_context(&pool, &url);
    (
        JobExecutionService::new(app_ctx, ExtensionRegistry::new(), config),
        pool,
    )
}

fn locked_config() -> SchedulerConfig {
    SchedulerConfig::with_system_admin()
}

#[tokio::test]
async fn a_peer_held_lock_makes_a_cluster_run_fail_as_contended() {
    let (svc, pool) = service(locked_config()).await;
    let peer = hold_peer_lock(&pool, WORKING_JOB).await;

    let batch = svc
        .run_jobs(
            &JobSelection::Names(vec![JobName::new(WORKING_JOB)]),
            &HashMap::new(),
        )
        .await
        .expect("batch");

    drop(peer.detach());
    assert_eq!(batch.contended, 1);
    assert_eq!(batch.failed, 1);
    let run = &batch.runs[0];
    assert!(run.lock_contended);
    assert!(!run.success);
    assert!(
        run.message
            .as_deref()
            .is_some_and(|m| m.contains("another replica")),
        "the operator must be told why the job did not run: {:?}",
        run.message
    );
}

#[tokio::test]
async fn a_bounded_wait_runs_the_job_once_the_peer_releases() {
    let (svc, pool) = service(locked_config()).await;
    let svc = svc.with_lock_wait(Duration::from_secs(10));
    let peer = hold_peer_lock(&pool, WORKING_JOB).await;

    let release = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        drop(peer.detach());
    });

    let report = svc
        .run_job(&JobName::new(WORKING_JOB), &HashMap::new())
        .await;
    release.await.expect("release task");

    assert!(!report.lock_contended, "the wait must outlast the peer");
    assert!(report.success, "{:?}", report.message);
}

#[tokio::test]
async fn a_node_job_runs_despite_a_peer_lock_on_its_name() {
    let (svc, pool) = service(locked_config()).await;
    let peer = hold_peer_lock(&pool, NODE_JOB).await;

    let report = svc.run_job(&JobName::new(NODE_JOB), &HashMap::new()).await;
    drop(peer.detach());

    assert!(!report.lock_contended);
    assert!(report.success, "{:?}", report.message);
}

#[tokio::test]
async fn without_the_distributed_lock_a_cluster_job_runs_unclaimed() {
    let mut config = locked_config();
    config.distributed_lock = false;
    let (svc, pool) = service(config).await;
    let peer = hold_peer_lock(&pool, WORKING_JOB).await;

    let report = svc
        .run_job(&JobName::new(WORKING_JOB), &HashMap::new())
        .await;
    drop(peer.detach());

    assert!(!report.lock_contended);
    assert!(report.success, "{:?}", report.message);
}
