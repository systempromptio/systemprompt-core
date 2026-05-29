//! Live async tests for `DatabaseContext` that require a real Postgres
//! connection. These run against `DATABASE_URL` set in the test environment.

use systemprompt_runtime::DatabaseContext;

const DB_URL: &str = "postgres://systemprompt_admin:3e00fcdac26b5b731829e8737515db8f@localhost:5432/systemprompt_cov_lift";

#[tokio::test]
async fn from_url_connects_successfully() {
    let ctx = DatabaseContext::from_url(DB_URL)
        .await
        .expect("DatabaseContext::from_url should succeed");

    let pool = ctx.db_pool();
    let _ = pool; // just verify it's accessible
}

#[tokio::test]
async fn from_url_pool_arc_clones_correctly() {
    let ctx = DatabaseContext::from_url(DB_URL)
        .await
        .expect("connect");

    let arc_pool = ctx.db_pool_arc();
    let arc_pool2 = ctx.db_pool_arc();

    assert!(
        std::sync::Arc::ptr_eq(&arc_pool, &arc_pool2),
        "both arcs should point to the same pool"
    );
}

#[tokio::test]
async fn from_url_db_pool_ref_and_arc_same_pointer() {
    let ctx = DatabaseContext::from_url(DB_URL)
        .await
        .expect("connect");

    let pool_ref = ctx.db_pool();
    let pool_arc = ctx.db_pool_arc();
    assert!(std::sync::Arc::ptr_eq(pool_ref, &pool_arc));
}

#[tokio::test]
async fn from_urls_without_write_url_connects() {
    let ctx = DatabaseContext::from_urls(DB_URL, None)
        .await
        .expect("from_urls without write url");
    let _ = ctx.db_pool();
}

#[tokio::test]
async fn from_urls_with_write_url_same_as_read_connects() {
    let ctx = DatabaseContext::from_urls(DB_URL, Some(DB_URL))
        .await
        .expect("from_urls with write url == read url");
    let _ = ctx.db_pool();
}

#[tokio::test]
async fn database_context_clone_shares_pool() {
    let ctx = DatabaseContext::from_url(DB_URL)
        .await
        .expect("connect");

    let cloned = ctx.clone();
    assert!(std::sync::Arc::ptr_eq(ctx.db_pool(), cloned.db_pool()));
}

#[tokio::test]
async fn database_context_debug_output() {
    let ctx = DatabaseContext::from_url(DB_URL)
        .await
        .expect("connect");

    let dbg = format!("{ctx:?}");
    assert!(dbg.contains("DatabaseContext"), "got: {dbg}");
}

#[tokio::test]
async fn from_url_invalid_returns_error() {
    let result = DatabaseContext::from_url("postgres://invalid_host_that_cannot_resolve:9999/nodb")
        .await;
    assert!(result.is_err(), "should fail with an invalid/unreachable URL");
}
