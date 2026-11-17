//! What `ContentExtension` contributes: its schemas, the two values it
//! provides (the content catalog and the content repositories), and its
//! default hooks in `ContentSlot`, built as the runtime builds an admitted
//! member's slot entry.

use std::any::TypeId;
use std::sync::Arc;

use systemprompt_content::{ContentExtension, ContentHooks, ContentRepositories};
use systemprompt_extension::contribute::{ContributionSink, build_slot};
use systemprompt_extension::{Contributions, Extension, StandingTag};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::Kernel;
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::{BackgroundTasks, DatabaseHandle, DynContentCatalogStats};

fn contributed() -> ContributionSink {
    let mut sink = ContributionSink::default();
    let mut c = Contributions::__new(ExtensionId::new("content"), &mut sink);
    ContentExtension
        .contribute(&mut c)
        .expect("the content member contributes");
    sink
}

async fn built_hooks() -> Arc<ContentHooks> {
    let sink = contributed();
    let pool = test_db_pool().await;
    let handle: Arc<dyn DatabaseHandle> = pool;
    let kernel = Kernel::new(handle, BackgroundTasks::new(), Dependencies::default());
    let inputs = sink
        .slots()
        .iter()
        .filter_map(|record| record.take_entry(ExtensionId::new("content"), StandingTag::Core))
        .collect();
    let built = build_slot(inputs, &kernel.resolved())
        .expect("the content slot builds")
        .expect("the content member fills the content slot");
    let mut state = Dependencies::default();
    built
        .insert_into(&mut state)
        .expect("an empty state holds no slot value");
    Arc::clone(
        state
            .get::<Arc<ContentHooks>>()
            .expect("the built hooks are inserted"),
    )
}

#[test]
fn test_metadata_id_and_name() {
    let meta = ContentExtension.metadata();
    assert_eq!(meta.id, "content");
    assert_eq!(meta.name, "Content");
    assert!(!meta.version.is_empty());
}

#[test]
fn test_schemas_include_core_tables() {
    let sink = contributed();
    let names: Vec<&str> = sink
        .schemas()
        .iter()
        .filter_map(|s| s.table.as_deref())
        .collect();
    assert_eq!(names.len(), 6, "{names:?}");
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
fn the_content_member_contributes_its_migrations() {
    assert!(!contributed().migrations().is_empty());
}

#[test]
fn the_content_member_provides_the_catalog_and_the_repositories() {
    let sink = contributed();
    let provided: Vec<TypeId> = sink.provides().iter().map(|p| p.type_id()).collect();
    assert_eq!(
        provided,
        [
            TypeId::of::<DynContentCatalogStats>(),
            TypeId::of::<Arc<ContentRepositories>>(),
        ]
    );
}

#[tokio::test]
async fn the_default_hooks_are_one_prerenderer_two_page_providers_and_one_renderer() {
    let hooks = built_hooks().await;
    assert_eq!(hooks.page_prerenderers().len(), 1, "{hooks:?}");
    assert_eq!(hooks.page_prerenderers()[0].page_type(), "homepage");
    assert_eq!(hooks.page_data_providers().len(), 2, "{hooks:?}");
    assert_eq!(hooks.component_renderers().len(), 1, "{hooks:?}");
    assert!(hooks.template_data_extenders().is_empty(), "{hooks:?}");
    assert!(hooks.frontmatter_processors().is_empty(), "{hooks:?}");
    assert!(hooks.content_data_providers().is_empty(), "{hooks:?}");
}

#[test]
fn no_reporting_capture_or_privacy_sql_is_registered() {
    assert!(
        contributed()
            .schemas()
            .iter()
            .all(|schema| !schema.sql.contains("reporting")),
        "the reporting projection is retired"
    );
}
