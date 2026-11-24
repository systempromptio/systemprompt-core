use std::sync::Arc;
use systemprompt_test_fixtures::AppContextSeeds as _;

use systemprompt_analytics::jobs::LinkFactsRollupJob;
use systemprompt_identifiers::{Actor, JobName, UserId};
use systemprompt_scheduler::JobConfig;
use systemprompt_scheduler::jobs::{
    BackfillRollupsJob, LogSummaryRollupJob, SessionBotTypeRollupJob, TraceFactsRollupJob,
    VertexDiscoveryJob,
};
use systemprompt_test_fixtures::{test_database_url, test_db_pool};
use systemprompt_traits::{Job, JobContext};

async fn ctx(pool: &systemprompt_database::DbPool, url: &str) -> JobContext {
    ctx_for(
        pool,
        systemprompt_test_fixtures::test_app_context(url).await,
    )
}

fn ctx_for(
    pool: &systemprompt_database::DbPool,
    app_ctx: Arc<systemprompt_runtime::AppContext>,
) -> JobContext {
    let dependencies = app_ctx
        .kernel()
        .state()
        .clone()
        .with(Arc::clone(app_ctx.app_paths_arc()))
        .with(Arc::clone(pool))
        .with(app_ctx);
    JobContext::new(
        Actor::job(UserId::new("projection-job-admin"), "test".to_owned()),
        dependencies,
    )
}

async fn assert_drains(job: &dyn Job) {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let context = ctx(&pool, &url).await;
    let result = job
        .execute(&context)
        .await
        .unwrap_or_else(|e| panic!("{} must drain an idle backlog: {e}", job.name()));
    assert!(result.success, "{} reports success", job.name());
    assert!(!job.description().is_empty());
}

#[tokio::test]
async fn trace_facts_rollup_drains() {
    assert_drains(&TraceFactsRollupJob).await;
}

#[tokio::test]
async fn analytics_event_link_facts_rollup_drains_both_projections() {
    assert_drains(&LinkFactsRollupJob).await;
}

#[tokio::test]
async fn backfill_rollups_drains_every_rollup() {
    assert_drains(&BackfillRollupsJob).await;
}

#[tokio::test]
async fn log_summary_rollup_drains() {
    assert_drains(&LogSummaryRollupJob).await;
}

#[tokio::test]
async fn session_bot_type_rollup_drains() {
    assert_drains(&SessionBotTypeRollupJob).await;
}

#[tokio::test]
async fn vertex_discovery_with_no_vertex_endpoints_scans_nothing() {
    let url = test_database_url();
    let (app, schedule) = crate::test_extension::scheduled_app(&url).await;
    let result = crate::test_extension::run_admitted_job(
        app,
        schedule,
        JobConfig::new(JobName::new(VertexDiscoveryJob.name())),
    )
    .await;
    assert!(result.success, "discovery runs: {:?}", result.message);
    assert_eq!(result.items_processed, Some(0));
}
