//! DB-backed `AccessControlSyncJob::execute` success run (non-destructive:
//! orphan deletion and overrides disabled so the shared baseline is
//! untouched) plus the YAML error arms of `AccessControlLocalSync`.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_models::AppPaths;
use systemprompt_models::profile::PathsConfig;
use systemprompt_sync::{AccessControlLocalSync, AccessControlSyncJob, SyncError};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{Job, JobContext};
use tempfile::TempDir;

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
    let actor = Actor::job(UserId::new("acl-sync-flow-test"), "test".to_owned());
    let db_pool_any: Arc<dyn Any + Send + Sync> = Arc::new(pool.clone());
    let app_context_any: Arc<dyn Any + Send + Sync> = Arc::new(());
    let app_paths_any: Arc<dyn Any + Send + Sync> = Arc::new(paths);
    JobContext::new(actor, db_pool_any, app_context_any, app_paths_any).with_parameters(params)
}

fn non_destructive_params(yaml_path: &str) -> HashMap<String, String> {
    HashMap::from([
        ("yaml_path".to_owned(), yaml_path.to_owned()),
        ("override_existing".to_owned(), "false".to_owned()),
        ("delete_orphans".to_owned(), "false".to_owned()),
    ])
}

#[tokio::test]
async fn execute_projects_empty_rule_set_successfully() {
    let pool = pool_or_skip!();
    ensure_test_bootstrap();
    let tmp = TempDir::new().expect("tempdir");
    let yaml = tmp.path().join("acl.yaml");
    std::fs::write(&yaml, "rules: []\n").expect("write yaml");

    let ctx = ctx_with(
        &pool,
        paths_for(tmp.path()),
        non_destructive_params(&yaml.to_string_lossy()),
    );
    let result = AccessControlSyncJob.execute(&ctx).await.expect("job");
    assert!(result.success);
    assert_eq!(result.items_failed, Some(0));
}

#[tokio::test]
async fn execute_resolves_relative_yaml_path_against_services() {
    let pool = pool_or_skip!();
    ensure_test_bootstrap();
    let tmp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("access-control")).expect("dir");
    std::fs::write(tmp.path().join("access-control/custom.yaml"), "rules: []\n")
        .expect("write yaml");

    let ctx = ctx_with(
        &pool,
        paths_for(tmp.path()),
        non_destructive_params("access-control/custom.yaml"),
    );
    let result = AccessControlSyncJob.execute(&ctx).await.expect("job");
    assert!(result.success);
}

#[tokio::test]
async fn execute_fails_when_yaml_missing_at_default_path() {
    let pool = pool_or_skip!();
    let tmp = TempDir::new().expect("tempdir");

    let params = HashMap::from([
        ("override_existing".to_owned(), "false".to_owned()),
        ("delete_orphans".to_owned(), "false".to_owned()),
    ]);
    let ctx = ctx_with(&pool, paths_for(tmp.path()), params);
    let err = AccessControlSyncJob
        .execute(&ctx)
        .await
        .expect_err("must fail");
    assert!(
        err.to_string().contains("Access-control config not found"),
        "{err}"
    );
}

#[tokio::test]
async fn sync_to_db_rejects_unparseable_yaml() {
    let pool = pool_or_skip!();
    let tmp = TempDir::new().expect("tempdir");
    let yaml = tmp.path().join("broken.yaml");
    std::fs::write(&yaml, "rules: [ {").expect("write yaml");

    let sync = AccessControlLocalSync::new(pool.clone(), yaml);
    let err = sync.sync_to_db(false, false).await.expect_err("must fail");
    match err {
        SyncError::InvalidInput(message) => {
            assert!(message.contains("AccessControlConfig"), "{message}");
        },
        other => panic!("expected InvalidInput, got {other:?}"),
    }
}
