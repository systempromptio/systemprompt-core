//! Live async tests for `DatabaseContext` that require a real Postgres
//! connection. They connect to the `DATABASE_URL` set in the test environment
//! (CI provisions one and migrates it); when the variable is unset the tests
//! fail.

use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::test_database_url;

#[tokio::test]
async fn from_url_connects_successfully() {
    let url = test_database_url();
    let ctx = DatabaseContext::from_url(&url)
        .await
        .expect("DatabaseContext::from_url should succeed");

    let pool = ctx.db_pool();
    let _ = pool; // just verify it's accessible
}

#[tokio::test]
async fn from_url_pool_arc_clones_correctly() {
    let url = test_database_url();
    let ctx = DatabaseContext::from_url(&url).await.expect("connect");

    let arc_pool = ctx.db_pool_arc();
    let arc_pool2 = ctx.db_pool_arc();

    assert!(
        std::sync::Arc::ptr_eq(&arc_pool, &arc_pool2),
        "both arcs should point to the same pool"
    );
}

#[tokio::test]
async fn from_url_db_pool_ref_and_arc_same_pointer() {
    let url = test_database_url();
    let ctx = DatabaseContext::from_url(&url).await.expect("connect");

    let pool_ref = ctx.db_pool();
    let pool_arc = ctx.db_pool_arc();
    assert!(std::sync::Arc::ptr_eq(pool_ref, &pool_arc));
}

#[tokio::test]
async fn from_urls_without_write_url_connects() {
    let url = test_database_url();
    let ctx = DatabaseContext::from_urls(&url, None)
        .await
        .expect("from_urls without write url");
    let _ = ctx.db_pool();
}

#[tokio::test]
async fn from_urls_with_write_url_same_as_read_connects() {
    let url = test_database_url();
    let ctx = DatabaseContext::from_urls(&url, Some(url.as_str()))
        .await
        .expect("from_urls with write url == read url");
    let _ = ctx.db_pool();
}

#[tokio::test]
async fn database_context_clone_shares_pool() {
    let url = test_database_url();
    let ctx = DatabaseContext::from_url(&url).await.expect("connect");

    let cloned = ctx.clone();
    assert!(std::sync::Arc::ptr_eq(ctx.db_pool(), cloned.db_pool()));
}


#[tokio::test]
async fn from_pool_wraps_the_given_pool_without_reconnecting() {
    let url = test_database_url();
    let seed = DatabaseContext::from_url(&url).await.expect("connect");
    let pool = seed.db_pool_arc();

    let ctx = DatabaseContext::from_pool(std::sync::Arc::clone(&pool));
    assert!(std::sync::Arc::ptr_eq(ctx.db_pool(), &pool));
}

#[tokio::test]
async fn from_url_invalid_returns_error() {
    let result =
        DatabaseContext::from_url("postgres://invalid_host_that_cannot_resolve:9999/nodb").await;
    assert!(
        result.is_err(),
        "should fail with an invalid/unreachable URL"
    );
}
