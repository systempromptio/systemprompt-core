//! `core content popular` against the database: the leaf parses its duration
//! window, ranks a source's content from the analytics report views and
//! refuses a malformed window.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_analytics_entry::cli::content_popular::{PopularArgs, execute_with_pool};
use systemprompt_database::DbPool;
use systemprompt_extension::cli::CliConfig;
use systemprompt_identifiers::SourceId;
use systemprompt_test_fixtures::test_db_pool;

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

async fn seed_content(pool: &DbPool, source: &str, slug: &str, title: &str) {
    sqlx::query(
        "INSERT INTO markdown_content (id, slug, title, description, body, author, \
         published_at, keywords, kind, source_id, version_hash) \
         VALUES ($1, $2, $3, 'A description', 'The body', 'author', NOW(), 'alpha', \
         'article', $4, 'hash-1')",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(slug)
    .bind(title)
    .bind(source)
    .execute(pool.pool().as_ref())
    .await
    .unwrap();
}

fn rows(out: &systemprompt_extension::cli::CommandOutput) -> Vec<serde_json::Value> {
    let artifact = serde_json::to_value(out.artifact()).expect("serialize table artifact");
    artifact["items"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("table artifact has items: {artifact}"))
}

#[tokio::test]
async fn popular_lists_the_sources_content_from_the_report_views() {
    let pool = test_db_pool().await;
    let source = format!("src-{}", uuid::Uuid::new_v4());
    let slug = format!("slug{}", uuid::Uuid::new_v4().simple());
    seed_content(&pool, &source, &slug, "Popular title").await;

    let out = execute_with_pool(
        PopularArgs {
            source: SourceId::new(source.clone()),
            since: "1w".to_owned(),
            limit: 5,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    let rows = rows(&out);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["title"], "Popular title");
    assert_eq!(rows[0]["slug"], slug.as_str());
    assert_eq!(rows[0]["source_id"], source.as_str());
    assert!(rows[0].get("category_id").is_none(), "{rows:?}");
}

#[tokio::test]
async fn popular_refuses_a_malformed_window() {
    let pool = test_db_pool().await;
    let err = execute_with_pool(
        PopularArgs {
            source: SourceId::new("any"),
            since: "not-a-duration".to_owned(),
            limit: 5,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Invalid duration format"), "{err}");
}
