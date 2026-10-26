//! Unit tests for `ContentExtension`.

use systemprompt_content::ContentExtension;
use systemprompt_extension::prelude::*;

#[test]
fn test_metadata_id_and_name() {
    let ext = ContentExtension;
    let meta = ext.metadata();
    assert_eq!(meta.id, "content");
    assert_eq!(meta.name, "Content");
    assert!(!meta.version.is_empty());
}

#[test]
fn test_dependencies_includes_users_and_analytics() {
    let ext = ContentExtension;
    let deps = ext.dependencies();
    assert!(deps.contains(&"users"));
    assert!(deps.contains(&"analytics"));
}

#[test]
fn test_schemas_count_six() {
    let ext = ContentExtension;
    let schemas = ext.schemas();
    assert_eq!(
        schemas
            .iter()
            .filter(|schema| schema.table.is_some())
            .count(),
        6
    );
}

#[test]
fn test_schemas_include_core_tables() {
    let ext = ContentExtension;
    let schemas = ext.schemas();
    let names: Vec<&str> = schemas.iter().filter_map(|s| s.table.as_deref()).collect();
    for expected in [
        "markdown_categories",
        "markdown_content",
        "markdown_fts",
        "content_performance_metrics",
        "campaign_links",
        "link_clicks",
    ] {
        assert!(
            names.contains(&expected),
            "missing schema table: {expected}"
        );
    }
}

#[tokio::test]
async fn test_page_prerenderers_includes_homepage() {
    let ctx = systemprompt_test_fixtures::fixture_extension_context(
        &systemprompt_test_fixtures::test_db_pool().await,
    );
    let ext = ContentExtension;
    let prerenderers = ext.page_prerenderers(ctx.as_ref());
    assert_eq!(prerenderers.len(), 1);
}

#[tokio::test]
async fn test_page_data_providers_two_branding_providers() {
    let ctx = systemprompt_test_fixtures::fixture_extension_context(
        &systemprompt_test_fixtures::test_db_pool().await,
    );
    let ext = ContentExtension;
    let providers = ext.page_data_providers(ctx.as_ref());
    assert_eq!(providers.len(), 2);
}

#[tokio::test]
async fn test_component_renderers_one_renderer() {
    let ctx = systemprompt_test_fixtures::fixture_extension_context(
        &systemprompt_test_fixtures::test_db_pool().await,
    );
    let ext = ContentExtension;
    let renderers = ext.component_renderers(ctx.as_ref());
    assert_eq!(renderers.len(), 1);
}

#[test]
fn no_reporting_capture_or_privacy_sql_is_registered() {
    assert!(
        ContentExtension
            .schemas()
            .iter()
            .all(|schema| !schema.sql.contains("reporting")),
        "the reporting projection is retired"
    );
}
