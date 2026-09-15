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
fn owner_capture_and_privacy_contracts_are_registered() {
    let schemas = ContentExtension.schemas();
    let capture: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("EXECUTE FUNCTION sp_capture_reporting_change")
        })
        .collect();
    assert_eq!(
        capture.len(),
        1,
        "owner capture SQL must be registered exactly once"
    );
    assert!(
        capture[0].sql.contains("reporting_source_markdown_content"),
        "missing reporting view: reporting_source_markdown_content"
    );
    let privacy: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("CREATE OR REPLACE FUNCTION public.lock_content_reporting_sources")
        })
        .collect();
    assert_eq!(
        privacy.len(),
        1,
        "owner privacy SQL must survive capture registration"
    );
}
