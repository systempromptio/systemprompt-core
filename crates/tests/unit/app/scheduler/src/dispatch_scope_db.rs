//! Tests for cluster- versus node-scoped job claims: a node-scoped job runs on
//! every replica and only de-duplicates against its own instance, while a
//! cluster-scoped job still yields to a peer-held advisory lock. DB-backed
//! tests skip when `DATABASE_URL` is
//! unset locally, and fail under `CI`.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use systemprompt_database::DbPool;
use systemprompt_models::services::scheduler::JobScope;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{JobConfig, SchedulerConfig, SchedulerRepository, SchedulerService};
use systemprompt_test_fixtures::{fixture_app_context_with_config, fixture_config};

use crate::test_jobs::{NODE_JOB, NODE_JOB_RUNS};

// Why: every test here shares one `scheduled_jobs` row and one run counter,
// so they must not interleave within the process.
static SERIALIZE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn context_for_instance(pool: &DbPool, url: &str, instance_id: &str) -> Arc<AppContext> {
    let mut config = fixture_config(url);
    config.instance_id = instance_id.to_owned();
    fixture_app_context_with_config(pool, config).expect("fixture AppContext")
}

fn bootstrap_config(jobs: Vec<JobConfig>) -> SchedulerConfig {
    SchedulerConfig {
        enabled: true,
        jobs,
        bootstrap_jobs: vec![NODE_JOB.to_owned()],
        distributed_lock: true,
    }
}

async fn dispatch_on(pool: &DbPool, url: &str, instance_id: &str, jobs: Vec<JobConfig>) {
    let app_ctx = context_for_instance(pool, url, instance_id);
    let svc = SchedulerService::new(bootstrap_config(jobs), Arc::clone(pool), app_ctx)
        .expect("SchedulerService::new");
    svc.run_bootstrap_jobs(None)
        .await
        .expect("bootstrap dispatch must not abort");
}

async fn seed_row(pool: &DbPool) -> SchedulerRepository {
    let repo = SchedulerRepository::new(pool).expect("repo");
    repo.upsert_job(NODE_JOB, "", true)
        .await
        .expect("seed scheduled_jobs row");
    let pg = pool.write_pool_arc().expect("write pool");
    sqlx::query!(
        "UPDATE scheduled_jobs SET last_run = NULL, last_instance_id = NULL WHERE job_name = $1",
        NODE_JOB
    )
    .execute(&*pg)
    .await
    .expect("reset dedupe columns");
    repo
}

mod node_scope {
    use super::*;

    #[tokio::test]
    async fn runs_on_every_replica_back_to_back() {
        let (pool, url) = systemprompt_test_fixtures::db_pool_or_skip!();
        let _guard = SERIALIZE.lock().await;
        let repo = seed_row(&pool).await;

        let before = NODE_JOB_RUNS.load(Ordering::SeqCst);
        dispatch_on(&pool, &url, "node-a", Vec::new()).await;
        dispatch_on(&pool, &url, "node-b", Vec::new()).await;

        assert_eq!(
            NODE_JOB_RUNS.load(Ordering::SeqCst) - before,
            2,
            "a node-scoped job must run on both replicas even within the dedupe window"
        );
        let row = repo
            .find_job(NODE_JOB)
            .await
            .expect("find_job")
            .expect("seeded row");
        assert_eq!(row.last_instance_id.as_deref(), Some("node-b"));
    }

    #[tokio::test]
    async fn same_replica_within_dedupe_window_is_skipped() {
        let (pool, url) = systemprompt_test_fixtures::db_pool_or_skip!();
        let _guard = SERIALIZE.lock().await;
        seed_row(&pool).await;

        let before = NODE_JOB_RUNS.load(Ordering::SeqCst);
        dispatch_on(&pool, &url, "node-same", Vec::new()).await;
        dispatch_on(&pool, &url, "node-same", Vec::new()).await;

        assert_eq!(
            NODE_JOB_RUNS.load(Ordering::SeqCst) - before,
            1,
            "the same replica must not re-run a node-scoped job within 900ms"
        );
    }
}

mod cluster_scope {
    use super::*;

    #[tokio::test]
    async fn config_override_to_cluster_yields_to_peer_held_lock() {
        let (pool, url) = systemprompt_test_fixtures::db_pool_or_skip!();
        let _guard = SERIALIZE.lock().await;
        let repo = seed_row(&pool).await;

        let pg = pool.write_pool_arc().expect("write pool");
        let mut peer = pg.acquire().await.expect("peer connection");
        let key: i64 = sqlx::query_scalar!(r#"SELECT hashtext($1)::bigint AS "key!""#, NODE_JOB)
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

        let before = NODE_JOB_RUNS.load(Ordering::SeqCst);
        let jobs = vec![JobConfig::new(NODE_JOB).with_scope(JobScope::Cluster)];
        dispatch_on(&pool, &url, "node-c", jobs).await;

        sqlx::query_scalar!("SELECT pg_advisory_unlock($1)", key)
            .fetch_one(peer.as_mut())
            .await
            .expect("peer unlock");

        assert_eq!(
            NODE_JOB_RUNS.load(Ordering::SeqCst),
            before,
            "a cluster-scoped job must skip while a peer holds the advisory lock"
        );
        let row = repo
            .find_job(NODE_JOB)
            .await
            .expect("find_job")
            .expect("seeded row");
        assert_eq!(
            row.last_instance_id, None,
            "a skipped dispatch must not stamp last_instance_id"
        );
    }
}
