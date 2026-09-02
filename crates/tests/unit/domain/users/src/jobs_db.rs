//! DB-backed execution paths for `CleanupAnonymousUsersJob::execute` and
//! `UserRateLimitPruneJob::execute`.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Duration, Utc};

use systemprompt_identifiers::{Actor, UserId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{Job, JobContext};
use systemprompt_users::UserRateLimitBucketRepository;
use systemprompt_users::jobs::{CleanupAnonymousUsersJob, UserRateLimitPruneJob};
use uuid::Uuid;

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

#[tokio::test]
async fn rate_limit_prune_drops_windows_older_than_retain_secs_and_keeps_the_rest() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = UserRateLimitBucketRepository::new(&pool).expect("repo");
    let user = UserId::new(format!("rl-prune-job-{}", Uuid::new_v4().simple()));
    let stale = Utc::now() - Duration::days(400);
    let live = Utc::now();
    repo.hit(&user, "job", stale).await.expect("stale hit");
    repo.hit(&user, "job", live).await.expect("live hit");

    // Why: the default 3600 s cutoff would also delete the live windows of
    // every other test sharing the database; a retention of 399 days confines
    // the sweep to this test's stale row.
    let retain_secs = Duration::days(399).num_seconds().to_string();
    let ctx = ctx_with_pool(Arc::new(pool.clone()))
        .with_parameters(HashMap::from([("retain_secs".to_owned(), retain_secs)]));
    let result = UserRateLimitPruneJob
        .execute(&ctx)
        .await
        .expect("job execute");

    assert!(result.success);
    assert!(
        result.items_processed.unwrap_or(0) >= 1,
        "the stale window must be counted as pruned"
    );
    assert_eq!(
        repo.hit(&user, "job", live).await.expect("live hit"),
        2,
        "a window inside the retention keeps its count"
    );
    assert_eq!(
        repo.hit(&user, "job", stale).await.expect("stale hit"),
        1,
        "the stale window was removed and restarts from zero"
    );

    let pg = pool.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM user_rate_limit_buckets WHERE user_id = $1")
        .bind(user.as_str())
        .execute(&*pg)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn rate_limit_prune_rejects_an_unparseable_retain_secs() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let ctx = ctx_with_pool(Arc::new(pool)).with_parameters(HashMap::from([(
        "retain_secs".to_owned(),
        "soon".to_owned(),
    )]));

    let err = UserRateLimitPruneJob
        .execute(&ctx)
        .await
        .expect_err("a non-numeric retention must fail rather than default silently");
    assert!(err.to_string().contains("retain_secs"), "{err}");
}

#[tokio::test]
async fn rate_limit_prune_fails_without_db_pool_in_context() {
    ensure_test_bootstrap();
    let ctx = ctx_with_pool(Arc::new(()));
    let err = UserRateLimitPruneJob
        .execute(&ctx)
        .await
        .expect_err("missing pool must fail");
    assert!(err.to_string().contains("DbPool"));
}
