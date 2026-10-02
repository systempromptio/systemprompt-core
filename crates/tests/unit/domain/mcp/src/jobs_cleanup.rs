//! Behaviour of the inventory-registered `mcp_session_cleanup` job: metadata,
//! a successful run over a live pool, and the context/pool error arms.

use std::sync::Arc;
use systemprompt_identifiers::Actor;
use systemprompt_provider_contracts::{Dependencies, Job, JobContext, ProviderError};
use systemprompt_test_fixtures::{closed_db_pool, fixture_user_id, test_db_pool};

fn cleanup_job() -> &'static dyn Job {
    inventory::iter::<&'static dyn Job>()
        .find(|j| j.name() == "mcp_session_cleanup")
        .copied()
        .expect("mcp_session_cleanup registered via submit_job!")
}

fn context_with(dependencies: Dependencies) -> JobContext {
    JobContext::new(Actor::user(fixture_user_id()), dependencies)
}

#[test]
fn job_metadata_names_the_schedule() {
    let job = cleanup_job();
    assert_eq!(job.name(), "mcp_session_cleanup");
    assert_eq!(job.schedule(), "0 */30 * * * *");
    assert!(job.description().contains("stale MCP sessions"));
}

#[tokio::test]
async fn execute_succeeds_against_live_pool() {
    let db = test_db_pool().await;
    let result = cleanup_job()
        .execute(&context_with(Dependencies::new().with(db)))
        .await
        .expect("cleanup runs");
    assert!(result.success);
}

#[tokio::test]
async fn execute_without_db_pool_in_context_fails() {
    let err = cleanup_job()
        .execute(&context_with(Dependencies::new().with(1_u8)))
        .await
        .expect_err("missing DbPool");
    assert!(matches!(err, ProviderError::MissingDependency(_)), "{err}");
}

#[tokio::test]
async fn execute_with_closed_pool_surfaces_query_error() {
    let db = closed_db_pool().await;
    let err = cleanup_job()
        .execute(&context_with(Dependencies::new().with(db)))
        .await
        .expect_err("closed pool");
    assert!(err.to_string().to_lowercase().contains("pool"));
}
