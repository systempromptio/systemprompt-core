//! DB-backed execution paths for `CleanupAnonymousUsersJob::execute`.

use std::any::Any;
use std::sync::Arc;

use systemprompt_identifiers::{Actor, UserId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{Job, JobContext};
use systemprompt_users::jobs::CleanupAnonymousUsersJob;

fn ctx_with_pool(db_pool_any: Arc<dyn Any + Send + Sync>) -> JobContext {
    let actor = Actor::job(UserId::new("users-jobs-db-test"), "test".to_owned());
    let app_context_any: Arc<dyn Any + Send + Sync> = Arc::new(());
    let app_paths_any: Arc<dyn Any + Send + Sync> = Arc::new(());
    JobContext::new(actor, db_pool_any, app_context_any, app_paths_any)
}

#[tokio::test]
async fn execute_succeeds_with_real_pool() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");

    let ctx = ctx_with_pool(Arc::new(pool));
    let result = CleanupAnonymousUsersJob
        .execute(&ctx)
        .await
        .expect("job execute");
    assert!(result.success);
    assert!(result.items_processed.is_some());
}

#[tokio::test]
async fn execute_fails_without_db_pool_in_context() {
    ensure_test_bootstrap();
    let ctx = ctx_with_pool(Arc::new(()));
    let err = CleanupAnonymousUsersJob
        .execute(&ctx)
        .await
        .expect_err("missing pool must fail");
    assert!(err.to_string().contains("DbPool"));
}

#[tokio::test]
async fn execute_fails_with_closed_pool() {
    ensure_test_bootstrap();
    let pool = systemprompt_test_fixtures::closed_db_pool().await;
    let ctx = ctx_with_pool(Arc::new(pool));
    let result: Result<_, _> = CleanupAnonymousUsersJob.execute(&ctx).await;
    assert!(result.is_err());
}
