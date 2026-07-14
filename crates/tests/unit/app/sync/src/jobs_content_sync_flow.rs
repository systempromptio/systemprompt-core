//! DB-backed `ContentSyncJob::execute` runs that reach the sync arms: an
//! added disk file ingested `to_db`, a DB-only row exported `to_disk`, and
//! the malformed-config error terminal. Each test namespaces its source id
//! so parallel writers cannot collide.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_content::models::CreateContentParams;
use systemprompt_content::repository::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, LocaleCode, SourceId, UserId};
use systemprompt_models::AppPaths;
use systemprompt_models::profile::PathsConfig;
use systemprompt_sync::{ContentSyncJob, compute_content_hash};
use systemprompt_test_fixtures::{closed_db_pool, fixture_database_url, fixture_db_pool};
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
    Actor::job(UserId::new("content-sync-flow-test"), "test".to_owned())
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

fn write_content_config(root: &std::path::Path, source_id: &SourceId, docs_path: &str) {
    let dir = root.join("content");
    std::fs::create_dir_all(&dir).expect("content dir");
    let body = format!(
        "content_sources:\n  docs:\n    path: \"{docs_path}\"\n    source_id: \"{source_id}\"\n    \
         category_id: \"cat-flow\"\n    enabled: true\n    allowed_content_types:\n      - \
         article\n"
    );
    std::fs::write(dir.join("config.yaml"), body).expect("write content config");
}

fn write_article(dir: &std::path::Path, slug: &str, title: &str, body: &str) {
    std::fs::create_dir_all(dir).expect("docs dir");
    let md = format!(
        "---\ntitle: \"{title}\"\nslug: \"{slug}\"\nauthor: \"Author\"\npublished_at: \
         \"2024-01-15\"\nkind: \"article\"\ndescription: \"d\"\n---\n\n{body}\n"
    );
    std::fs::write(dir.join(format!("{slug}.md")), md).expect("write md");
}

async fn seed_db_article(pool: &DbPool, source: &SourceId, slug: &str, title: &str, body: &str) {
    let repo = ContentRepository::new(pool).expect("repo");
    let params = CreateContentParams::new(
        slug.to_owned(),
        title.to_owned(),
        "desc".to_owned(),
        body.to_owned(),
        source.clone(),
    )
    .with_kind("article".to_owned())
    .with_version_hash(compute_content_hash(body, title));
    repo.create(&params).await.expect("seed content");
}

async fn cleanup(pool: &DbPool, source: &SourceId) {
    let repo = ContentRepository::new(pool).expect("repo");
    repo.delete_by_source(source).await.expect("cleanup");
}

#[tokio::test]
async fn execute_to_db_ingests_added_disk_content() {
    let pool = pool_or_skip!();
    let tmp = TempDir::new().expect("tempdir");
    let source_id = SourceId::new(format!("flow-db-{}", Uuid::new_v4()));
    let slug = format!("flow-added-{}", Uuid::new_v4().simple());

    write_content_config(tmp.path(), &source_id, "docs");
    write_article(&tmp.path().join("docs"), &slug, "Added", "Fresh body");

    let params = HashMap::from([("direction".to_owned(), "to_db".to_owned())]);
    let ctx = ctx_with(&pool, paths_for(tmp.path()), params);
    let result = ContentSyncJob.execute(&ctx).await.expect("job result");
    assert!(result.success);

    let repo = ContentRepository::new(&pool).expect("repo");
    let stored = repo
        .get_by_source_and_slug(&source_id, &slug, &LocaleCode::new("en"))
        .await
        .expect("query")
        .expect("ingested row");
    assert_eq!(stored.title, "Added");

    cleanup(&pool, &source_id).await;
}

#[tokio::test]
async fn execute_to_disk_exports_db_only_content() {
    let pool = pool_or_skip!();
    let tmp = TempDir::new().expect("tempdir");
    let source_id = SourceId::new(format!("flow-disk-{}", Uuid::new_v4()));
    let slug = format!("flow-removed-{}", Uuid::new_v4().simple());

    let docs = tmp.path().join("docs");
    std::fs::create_dir_all(&docs).expect("docs dir");
    write_content_config(tmp.path(), &source_id, &docs.to_string_lossy());
    seed_db_article(&pool, &source_id, &slug, "OnlyInDb", "Database body").await;

    let params = HashMap::from([("direction".to_owned(), "to_disk".to_owned())]);
    let ctx = ctx_with(&pool, paths_for(tmp.path()), params);
    let result = ContentSyncJob.execute(&ctx).await.expect("job result");
    assert!(result.success);

    let exported = docs.join(format!("{slug}.md"));
    let written = std::fs::read_to_string(&exported).expect("exported file");
    assert!(written.contains("Database body"), "{written}");

    cleanup(&pool, &source_id).await;
}

#[tokio::test]
async fn execute_in_sync_tree_reports_no_changes() {
    let pool = pool_or_skip!();
    let tmp = TempDir::new().expect("tempdir");
    let source_id = SourceId::new(format!("flow-clean-{}", Uuid::new_v4()));
    let slug = format!("flow-clean-{}", Uuid::new_v4().simple());

    write_content_config(tmp.path(), &source_id, "docs");
    write_article(&tmp.path().join("docs"), &slug, "Same", "Same body");
    seed_db_article(&pool, &source_id, &slug, "Same", "Same body").await;

    let to_db = HashMap::from([("direction".to_owned(), "to_db".to_owned())]);
    let ctx = ctx_with(&pool, paths_for(tmp.path()), to_db);
    let result = ContentSyncJob.execute(&ctx).await.expect("in-sync run");
    assert!(result.success);
    assert_eq!(result.message.as_deref(), Some("Content is in sync"));

    cleanup(&pool, &source_id).await;
}

#[tokio::test]
async fn execute_fails_without_app_paths_in_context() {
    let pool = pool_or_skip!();
    let db_pool_any: Arc<dyn Any + Send + Sync> = Arc::new(pool.clone());
    let app_context_any: Arc<dyn Any + Send + Sync> = Arc::new(());
    let app_paths_any: Arc<dyn Any + Send + Sync> = Arc::new(());
    let ctx = JobContext::new(actor(), db_pool_any, app_context_any, app_paths_any);

    let err = ContentSyncJob.execute(&ctx).await.expect_err("must fail");
    assert!(err.to_string().contains("AppPaths not available"), "{err}");
}

#[tokio::test]
async fn execute_maps_diff_failure_from_closed_pool() {
    let pool = closed_db_pool().await;
    let tmp = TempDir::new().expect("tempdir");
    let source_id = SourceId::new(format!("flow-closed-{}", Uuid::new_v4()));
    write_content_config(tmp.path(), &source_id, "docs");
    write_article(
        &tmp.path().join("docs"),
        &format!("flow-closed-{}", Uuid::new_v4().simple()),
        "T",
        "b",
    );

    let ctx = ctx_with(&pool, paths_for(tmp.path()), HashMap::new());
    let err = ContentSyncJob.execute(&ctx).await.expect_err("must fail");
    assert!(
        err.to_string()
            .contains("Failed to calculate diff for source docs"),
        "{err}"
    );
}

#[tokio::test]
async fn execute_errors_when_config_path_is_unreadable() {
    let pool = pool_or_skip!();
    let tmp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("content/config.yaml")).expect("dir as config");

    let ctx = ctx_with(&pool, paths_for(tmp.path()), HashMap::new());
    let err = ContentSyncJob.execute(&ctx).await.expect_err("must fail");
    assert!(
        err.to_string().contains("Failed to read content config"),
        "{err}"
    );
}

#[tokio::test]
async fn execute_rejects_malformed_content_config() {
    let pool = pool_or_skip!();
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().join("content");
    std::fs::create_dir_all(&dir).expect("content dir");
    std::fs::write(dir.join("config.yaml"), ": not yaml: [").expect("write config");

    let ctx = ctx_with(&pool, paths_for(tmp.path()), HashMap::new());
    let err = ContentSyncJob.execute(&ctx).await.expect_err("must fail");
    assert!(err.to_string().to_lowercase().contains("configuration"));
}
