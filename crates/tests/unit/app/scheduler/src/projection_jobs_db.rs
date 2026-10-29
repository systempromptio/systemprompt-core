use std::sync::Arc;

use systemprompt_identifiers::{Actor, UserId};
use systemprompt_scheduler::jobs::{
    AiRequestToolFlagRollupJob, AnalyticsEventLinkFactsRollupJob, BackfillRollupsJob,
    BackfillSessionGeoJob, InventoryRefreshJob, InventoryRefreshMemo, LogSummaryRollupJob,
    SessionBotTypeRollupJob, ToolRowFactsRollupJob, TraceFactsRollupJob,
    UserContextCountersRollupJob, VertexDiscoveryJob,
};
use systemprompt_test_fixtures::{
    seed_user_row, test_app_context, test_database_url, test_db_pool,
};
use systemprompt_traits::{Dependencies, Job, JobContext};

fn ctx(pool: &systemprompt_database::DbPool, url: &str) -> JobContext {
    ctx_for(pool, test_app_context(pool, url))
}

fn ctx_for(
    pool: &systemprompt_database::DbPool,
    app_ctx: Arc<systemprompt_runtime::AppContext>,
) -> JobContext {
    let dependencies = Dependencies::new()
        .with(Arc::clone(app_ctx.app_paths_arc()))
        .with(Arc::clone(pool))
        .with(Arc::new(InventoryRefreshMemo::new()))
        .with(app_ctx);
    JobContext::new(
        Actor::job(UserId::new("projection-job-admin"), "test".to_owned()),
        dependencies,
    )
}

async fn assert_drains(job: &dyn Job) {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let context = ctx(&pool, &url);
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
    assert_drains(&AnalyticsEventLinkFactsRollupJob).await;
}

#[tokio::test]
async fn tool_row_facts_rollup_drains() {
    assert_drains(&ToolRowFactsRollupJob).await;
}

#[tokio::test]
async fn ai_request_tool_flag_rollup_drains() {
    assert_drains(&AiRequestToolFlagRollupJob).await;
}

#[tokio::test]
async fn backfill_rollups_drains_every_rollup() {
    assert_drains(&BackfillRollupsJob).await;
}

#[tokio::test]
async fn backfill_session_geo_drains() {
    assert_drains(&BackfillSessionGeoJob).await;
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
async fn user_context_counters_rollup_drains() {
    assert_drains(&UserContextCountersRollupJob).await;
}

#[tokio::test]
async fn inventory_refresh_runs_twice_with_a_shared_memo() {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let app_ctx = test_app_context(&pool, &url);
    seed_user_row(
        &pool,
        app_ctx.system_admin().id(),
        "inventory-admin@example.test",
    )
    .await
    .expect("seed system admin");
    let context = ctx_for(&pool, app_ctx);
    let job = InventoryRefreshJob;
    let first = job.execute(&context).await.expect("first refresh");
    assert!(first.success);
    let second = job.execute(&context).await.expect("second refresh");
    assert!(second.success);
    assert_eq!(job.name(), "managed_inventory_refresh");
}

#[tokio::test]
async fn vertex_discovery_with_no_vertex_endpoints_scans_nothing() {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let context = ctx(&pool, &url);
    let result = VertexDiscoveryJob
        .execute(&context)
        .await
        .expect("discovery runs");
    assert!(result.success);
    assert_eq!(result.items_processed, Some(0));
}
