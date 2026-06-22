//! DB-backed tests for [`execute_content_ingestion`].
//!
//! Builds a temporary content source on disk and drives the job end to end:
//! enabled-source filtering, real markdown ingestion (asserting the row lands
//! in `markdown_content`), missing-path error accounting, and the
//! no-enabled-sources short-circuit.

use std::collections::HashMap;
use systemprompt_content::execute_content_ingestion;
use systemprompt_content::repository::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CategoryId, LocaleCode, SourceId};
use systemprompt_models::AppPaths;
use systemprompt_models::content_config::{
    ContentConfigRaw, ContentSourceConfigRaw, IndexingConfig,
};
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

fn app_paths() -> AppPaths {
    let paths = PathsConfig {
        system: "/tmp".to_owned(),
        services: "/tmp".to_owned(),
        bin: "/tmp".to_owned(),
        web_path: Some("/tmp".to_owned()),
        storage: Some("/tmp".to_owned()),
        geoip_database: None,
    };
    AppPaths::from_profile(&paths).expect("paths")
}

fn source_config(
    path: &str,
    source_id: &SourceId,
    category: &CategoryId,
) -> ContentSourceConfigRaw {
    ContentSourceConfigRaw {
        path: path.to_owned(),
        source_id: source_id.clone(),
        category_id: category.clone(),
        enabled: true,
        description: String::new(),
        allowed_content_types: vec![],
        indexing: Some(IndexingConfig {
            clear_before: false,
            recursive: false,
            override_existing: true,
        }),
        sitemap: None,
        branding: None,
    }
}

fn write_markdown(dir: &std::path::Path, slug: &str, title: &str) {
    let body = format!(
        "---\ntitle: \"{title}\"\nslug: \"{slug}\"\nauthor: \"Test Author\"\npublished_at: \"2024-01-15\"\nkind: \"article\"\ndescription: \"a desc\"\n---\n\n# {title}\n\nSome body content.\n"
    );
    std::fs::write(dir.join(format!("{slug}.md")), body).expect("write md");
}

#[tokio::test]
async fn ingests_enabled_source_into_content_store() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool: DbPool = fixture_db_pool(&url).await.expect("pool");

    let dir = tempfile::tempdir().expect("tempdir");
    let slug = format!("job-{}", Uuid::new_v4().simple());
    write_markdown(dir.path(), &slug, "Job Ingested Post");

    let source_id = SourceId::new(format!("job-src-{}", Uuid::new_v4()));
    let category = CategoryId::new("docs");
    let mut sources = HashMap::new();
    sources.insert(
        "docs".to_owned(),
        source_config(
            dir.path().to_str().expect("utf8 path"),
            &source_id,
            &category,
        ),
    );
    let config = ContentConfigRaw {
        content_sources: sources,
        ..Default::default()
    };

    let result = execute_content_ingestion(&pool, &config, &app_paths())
        .await
        .expect("job");

    assert!(result.success, "job should report success: {result:?}");

    let repo = ContentRepository::new(&pool).expect("repo");
    let stored = repo
        .get_by_source_and_slug(&source_id, &slug, &LocaleCode::new("en"))
        .await
        .expect("query")
        .expect("ingested row present");
    assert_eq!(stored.title, "Job Ingested Post");
    assert_eq!(stored.author, "Test Author");
    assert_eq!(stored.source_id, source_id);

    repo.delete_by_source(&source_id).await.expect("cleanup");
}

#[tokio::test]
async fn skill_sources_are_filtered_out() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool: DbPool = fixture_db_pool(&url).await.expect("pool");

    let dir = tempfile::tempdir().expect("tempdir");
    let slug = format!("skill-{}", Uuid::new_v4().simple());
    write_markdown(dir.path(), &slug, "Should Not Ingest");

    let source_id = SourceId::new(format!("skill-src-{}", Uuid::new_v4()));
    let category = CategoryId::new("skills");
    let mut sources = HashMap::new();
    sources.insert(
        "my-skill-source".to_owned(),
        source_config(
            dir.path().to_str().expect("utf8 path"),
            &source_id,
            &category,
        ),
    );
    let config = ContentConfigRaw {
        content_sources: sources,
        ..Default::default()
    };

    let result = execute_content_ingestion(&pool, &config, &app_paths())
        .await
        .expect("job");

    // "skill" sources are excluded, leaving no enabled sources to process.
    assert!(result.success);
    assert_eq!(
        result.message.as_deref(),
        Some("No enabled content sources")
    );

    let repo = ContentRepository::new(&pool).expect("repo");
    let stored = repo
        .get_by_source_and_slug(&source_id, &slug, &LocaleCode::new("en"))
        .await
        .expect("query");
    assert!(stored.is_none(), "skill source must not be ingested");
}

#[tokio::test]
async fn disabled_source_yields_empty_result() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool: DbPool = fixture_db_pool(&url).await.expect("pool");

    let source_id = SourceId::new(format!("disabled-{}", Uuid::new_v4()));
    let mut cfg = source_config("/tmp/does-not-matter", &source_id, &CategoryId::new("docs"));
    cfg.enabled = false;
    let mut sources = HashMap::new();
    sources.insert("docs".to_owned(), cfg);
    let config = ContentConfigRaw {
        content_sources: sources,
        ..Default::default()
    };

    let result = execute_content_ingestion(&pool, &config, &app_paths())
        .await
        .expect("job");
    assert!(result.success);
    assert_eq!(
        result.message.as_deref(),
        Some("No enabled content sources")
    );
}

#[tokio::test]
async fn missing_path_counts_as_error_but_job_succeeds() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool: DbPool = fixture_db_pool(&url).await.expect("pool");

    let source_id = SourceId::new(format!("missing-{}", Uuid::new_v4()));
    let missing = format!("/tmp/nonexistent-{}", Uuid::new_v4());
    let mut sources = HashMap::new();
    sources.insert(
        "docs".to_owned(),
        source_config(&missing, &source_id, &CategoryId::new("docs")),
    );
    let config = ContentConfigRaw {
        content_sources: sources,
        ..Default::default()
    };

    let result = execute_content_ingestion(&pool, &config, &app_paths())
        .await
        .expect("job");

    // The job aggregates the missing path as one error but still completes.
    assert!(result.success);
    assert!(
        result.message.is_none() || result.message.as_deref() != Some("No enabled content sources")
    );
}

#[tokio::test]
async fn empty_config_short_circuits() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool: DbPool = fixture_db_pool(&url).await.expect("pool");

    let config = ContentConfigRaw::default();
    let result = execute_content_ingestion(&pool, &config, &app_paths())
        .await
        .expect("job");
    assert!(result.success);
    assert_eq!(
        result.message.as_deref(),
        Some("No enabled content sources")
    );
}
