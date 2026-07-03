//! DB-backed execution paths for [`ContentSyncJob::execute`] that terminate in
//! a successful [`JobResult`] rather than an error.
//!
//! `jobs_content_sync` drives the early error branches (missing `DbPool`, bad
//! `direction`, absent content config). These tests instead stand up a real
//! `content/config.yaml` under a temp services tree so the job loads its source
//! list and reaches its "No enabled content sources" success terminal. The
//! enabled-source path additionally reads the shared content tables, so it is
//! left to the DB-grouped `local_content_sync` suite rather than duplicated
//! here where it would race parallel writers. Tests early-return when
//! `DATABASE_URL` is unset.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_models::AppPaths;
use systemprompt_models::profile::PathsConfig;
use systemprompt_sync::ContentSyncJob;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use systemprompt_traits::{Job, JobContext};
use tempfile::TempDir;
use uuid::Uuid;

macro_rules! pool_or_skip {
    () => {{
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(pool) = fixture_db_pool(&url).await else {
            return;
        };
        pool
    }};
}

fn actor() -> Actor {
    Actor::job(UserId::new("content-sync-db-test"), "test".to_owned())
}

fn paths_for(root: &std::path::Path) -> Arc<AppPaths> {
    let s = root.to_string_lossy().to_string();
    let cfg = PathsConfig {
        system: s.clone(),
        services: s.clone(),
        bin: s.clone(),
        web_path: Some(s.clone()),
        storage: Some(s),
        geoip_database: None,
    };
    Arc::new(AppPaths::from_profile(&cfg).expect("app paths"))
}

fn ctx_with(pool: &DbPool, paths: Arc<AppPaths>, params: HashMap<String, String>) -> JobContext {
    let db_pool_any: Arc<dyn Any + Send + Sync> = Arc::new(pool.clone());
    let app_context_any: Arc<dyn Any + Send + Sync> = Arc::new(());
    let app_paths_any: Arc<dyn Any + Send + Sync> = Arc::new(paths);
    JobContext::new(actor(), db_pool_any, app_context_any, app_paths_any).with_parameters(params)
}

fn write_content_config(root: &std::path::Path, body: &str) {
    let dir = root.join("content");
    std::fs::create_dir_all(&dir).expect("content dir");
    std::fs::write(dir.join("config.yaml"), body).expect("write content config");
}

#[tokio::test]
async fn execute_reports_no_enabled_sources() {
    let pool = pool_or_skip!();
    let tmp = TempDir::new().expect("tempdir");
    let source_id = format!("disabled-{}", Uuid::new_v4());
    let body = format!(
        "content_sources:\n  only:\n    path: \"docs\"\n    source_id: \"{source_id}\"\n    \
         category_id: \"cat-x\"\n    enabled: false\n    allowed_content_types:\n      - article\n"
    );
    write_content_config(tmp.path(), &body);

    let ctx = ctx_with(&pool, paths_for(tmp.path()), HashMap::new());
    let result = ContentSyncJob
        .execute(&ctx)
        .await
        .expect("job with only disabled sources succeeds");
    assert!(result.success, "expected a successful JobResult");
    let message = result.message.unwrap_or_default();
    assert!(
        message.contains("No enabled content sources"),
        "unexpected message: {message}"
    );
}
