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
    assert!(deps.iter().any(|d| *d == "users"));
    assert!(deps.iter().any(|d| *d == "analytics"));
}

#[test]
fn test_schemas_count_seven() {
    let ext = ContentExtension;
    let schemas = ext.schemas();
    assert_eq!(schemas.len(), 7);
}

#[test]
fn test_schemas_include_core_tables() {
    let ext = ContentExtension;
    let schemas = ext.schemas();
    let names: Vec<&str> = schemas.iter().map(|s| s.table.as_str()).collect();
    for expected in [
        "markdown_categories",
        "markdown_content",
        "markdown_fts",
        "content_performance_metrics",
        "campaign_links",
        "link_clicks",
        "link_analytics_views",
    ] {
        assert!(
            names.contains(&expected),
            "missing schema table: {expected}"
        );
    }
}

#[test]
fn test_page_prerenderers_includes_homepage() {
    let ext = ContentExtension;
    let prerenderers = ext.page_prerenderers();
    assert_eq!(prerenderers.len(), 1);
}

#[test]
fn test_page_data_providers_two_branding_providers() {
    let ext = ContentExtension;
    let providers = ext.page_data_providers();
    assert_eq!(providers.len(), 2);
}

#[test]
fn test_component_renderers_one_renderer() {
    let ext = ContentExtension;
    let renderers = ext.component_renderers();
    assert_eq!(renderers.len(), 1);
}

#[test]
fn test_clone_copy_default() {
    let a = ContentExtension;
    let b = a;
    assert_eq!(a.metadata().id, b.metadata().id);
    let _c: ContentExtension = ContentExtension::default();
}
