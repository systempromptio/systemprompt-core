//! Context-precondition branches for [`AccessControlSyncJob::execute`].
//!
//! The job reads a `DbPool` and `AppPaths` out of its [`JobContext`] before
//! doing any work. These tests drive the two `ok_or_else` guards: a context
//! whose type-erased slots do not downcast to `DbPool`, and one with a valid
//! pool but a bogus `AppPaths` slot. The full ingest path needs a provisioned
//! services tree and is covered by integration coverage. The AppPaths test
//! early-returns when `DATABASE_URL` is unset.

use std::any::Any;
use std::sync::Arc;

use systemprompt_identifiers::{Actor, UserId};
use systemprompt_sync::AccessControlSyncJob;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use systemprompt_traits::{Job, JobContext};

fn actor() -> Actor {
    Actor::job(UserId::new("acl-sync-db-test"), "test".to_owned())
}

#[tokio::test]
async fn execute_fails_without_db_pool_in_context() {
    let bogus: Arc<dyn Any + Send + Sync> = Arc::new(String::from("nope"));
    let ctx = JobContext::new(
        actor(),
        Arc::clone(&bogus),
        Arc::clone(&bogus),
        Arc::clone(&bogus),
    );

    let err = AccessControlSyncJob
        .execute(&ctx)
        .await
        .expect_err("missing DbPool must surface a configuration error");
    assert!(
        format!("{err}").contains("DbPool"),
        "error should name the missing DbPool: {err}"
    );
}

#[tokio::test]
async fn execute_fails_without_app_paths_in_context() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(pool) = fixture_db_pool(&url).await else {
        return;
    };

    let db_pool_any: Arc<dyn Any + Send + Sync> = Arc::new(pool.clone());
    let bogus: Arc<dyn Any + Send + Sync> = Arc::new(String::from("nope"));
    let ctx = JobContext::new(actor(), db_pool_any, Arc::clone(&bogus), bogus);

    let err = AccessControlSyncJob
        .execute(&ctx)
        .await
        .expect_err("missing AppPaths must surface a configuration error");
    assert!(
        format!("{err}").contains("AppPaths"),
        "error should name the missing AppPaths: {err}"
    );
    drop(pool);
}
