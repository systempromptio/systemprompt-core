//! Tests for [`ContentSyncJob::execute`].
//!
//! Drives the job's early-exit and error branches: a context missing the
//! `DbPool`, an invalid `direction` parameter, and a valid direction whose
//! content config is absent (the fixture `AppPaths` points at a directory with
//! no `content.yaml`). The happy path requires a fully provisioned services
//! tree and is covered by integration coverage elsewhere. DB-backed tests
//! early-return when `DATABASE_URL` is unset.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_identifiers::{Actor, UserId};
use systemprompt_sync::ContentSyncJob;
use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{Job, JobContext};

fn actor() -> Actor {
    Actor::job(UserId::new("content-sync-test"), "test".to_owned())
}

macro_rules! pool_or_skip {
    () => {{
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(pool) = fixture_db_pool(&url).await else {
            return;
        };
        (pool, url)
    }};
}

fn ctx_with(
    pool: &systemprompt_database::DbPool,
    url: &str,
    params: HashMap<String, String>,
) -> JobContext {
    let app_ctx = fixture_app_context(pool, url).expect("fixture AppContext");
    let app_paths_any: Arc<dyn std::any::Any + Send + Sync> =
        Arc::new(Arc::clone(app_ctx.app_paths_arc()));
    let db_pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(Arc::clone(pool));
    let app_context_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(app_ctx);
    JobContext::new(actor(), db_pool_any, app_context_any, app_paths_any).with_parameters(params)
}

#[tokio::test]
async fn execute_fails_without_db_pool_in_context() {
    // Type-erased Arcs that do not downcast to DbPool / AppPaths.
    let bogus: Arc<dyn std::any::Any + Send + Sync> = Arc::new(String::from("nope"));
    let ctx = JobContext::new(
        actor(),
        Arc::clone(&bogus),
        Arc::clone(&bogus),
        Arc::clone(&bogus),
    );

    let err = ContentSyncJob
        .execute(&ctx)
        .await
        .expect_err("missing DbPool must surface a configuration error");
    assert!(
        format!("{err}").contains("DbPool"),
        "error should mention the missing DbPool: {err}"
    );
}

#[tokio::test]
async fn execute_rejects_invalid_direction_parameter() {
    let (pool, url) = pool_or_skip!();
    let mut params = HashMap::new();
    params.insert("direction".to_owned(), "sideways".to_owned());
    let ctx = ctx_with(&pool, &url, params);

    let err = ContentSyncJob
        .execute(&ctx)
        .await
        .expect_err("invalid direction must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("sideways") || msg.to_lowercase().contains("direction"),
        "error should reference the bad direction: {msg}"
    );
}

#[tokio::test]
async fn execute_errors_when_content_config_missing() {
    let (pool, url) = pool_or_skip!();
    // Default direction (to_db) is valid, but the fixture services path has no
    // content config, so source loading fails.
    let ctx = ctx_with(&pool, &url, HashMap::new());

    let err = ContentSyncJob
        .execute(&ctx)
        .await
        .expect_err("absent content config must error");
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("content config") || msg.to_lowercase().contains("not found"),
        "error should reference the missing content config: {msg}"
    );
}
