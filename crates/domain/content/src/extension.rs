//! The content member: its schemas and migrations, its default content hooks
//! (the homepage prerenderer, the branding and list-branding page data
//! providers and the list-items component renderer) in [`ContentSlot`], and
//! the provided state: the content catalog
//! (`DynContentCatalogStats`), which static-content serving and analytics
//! read, the content repositories (`Arc<ContentRepositories>`), which the
//! `content-http` routes read, and the content–file links
//! (`DynContentFileLinks`, open as the repository is), which the
//! `content files` commands read; the content-audience report port
//! (`DynContentAudienceReport`, open), which the analytics content commands
//! read; and the audience ranking (`DynContentPopularity`, open) the content
//! prerender orders by, provided after the early provides so it reads the
//! viewer counts (`DynContentAudienceCounts`) the analytics member provides
//! when that member is admitted. The links read their stored files through
//! the `DynLinkedFileSource` the kernel's `files` member provides, which
//! this member requires.
//!
//! It owns the objects that cannot exist without its tables: the
//! `content_files` links (keyed to a `files` row, which the kernel always
//! links) and the `report_markdown_content` view the analytics reports read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::content_files::{DynContentFileLinks, DynLinkedFileSource};
use systemprompt_extension::prelude::*;
use systemprompt_traits::usage_reports::content::{
    DynContentAudienceCounts, DynContentAudienceReport,
};
use systemprompt_traits::{DynContentCatalogStats, DynContentPopularity};

use crate::branding_provider::default_branding_provider;
use crate::contract::ContentSlot;
use crate::homepage_prerenderer::default_homepage_prerenderer;
use crate::list_branding_provider::default_list_branding_provider;
use crate::list_items_renderer::default_list_items_renderer;
use crate::repository::{
    ContentAudienceReportRepository, ContentFileRepository, ContentRepositories, ContentRepository,
};
use crate::services::ContentPopularityService;

#[derive(Debug, Clone, Copy, Default)]
pub struct ContentExtension;

impl Extension for ContentExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "content",
            name: "Content",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.domain_config(systemprompt_manifest::validators::ContentConfigValidator::boxed);
        for schema in schemas() {
            c.schema(schema);
        }
        c.migrations(extension_migrations!())
            .provide(|r: &Resolved<'_>| {
                let db = r.get::<DbPool>()?;
                let catalog: DynContentCatalogStats = Arc::new(ContentRepository::new(&db));
                Ok(catalog)
            })
            .provide(|r: &Resolved<'_>| {
                let db = r.get::<DbPool>()?;
                Ok(Arc::new(ContentRepositories::new(&db)))
            })
            .requires::<DynLinkedFileSource>()
            .provide(|r: &Resolved<'_>| {
                let links: DynContentFileLinks = Arc::new(ContentFileRepository::new(
                    &r.get::<DbPool>()?,
                    r.get::<DynLinkedFileSource>()?,
                ));
                Ok(links)
            })
            .provide(|r: &Resolved<'_>| {
                let report: DynContentAudienceReport =
                    Arc::new(ContentAudienceReportRepository::new(&r.get::<DbPool>()?));
                Ok(report)
            })
            .provide_after_governance(|r: &Resolved<'_>| {
                let popularity: DynContentPopularity = Arc::new(ContentPopularityService::new(
                    ContentAudienceReportRepository::new(&r.get::<DbPool>()?),
                    r.optional::<DynContentAudienceCounts>(),
                ));
                Ok(popularity)
            });
        c.slot::<ContentSlot>()
            .page_prerenderer(|_: &Resolved<'_>| Ok(Some(default_homepage_prerenderer())))
            .page_data_provider(|_: &Resolved<'_>| Ok(Some(default_branding_provider())))
            .page_data_provider(|_: &Resolved<'_>| Ok(Some(default_list_branding_provider())))
            .component_renderer(|_: &Resolved<'_>| Ok(Some(default_list_items_renderer())));
        c.owner_reassignment(OwnerReassignment {
            table: "link_clicks",
            column: "user_id",
            action: ReassignAction::UPDATE,
        });
        Ok(())
    }
}

fn schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "markdown_categories",
            include_str!("../schema/markdown_categories.sql"),
        )
        .with_required_columns(vec!["id".into(), "name".into(), "slug".into()]),
        SchemaDefinition::new(
            "markdown_content",
            include_str!("../schema/markdown_content.sql"),
        )
        .with_required_columns(vec!["id".into(), "slug".into(), "title".into()]),
        SchemaDefinition::new("markdown_fts", include_str!("../schema/markdown_fts.sql")),
        SchemaDefinition::new(
            "content_performance_metrics",
            include_str!("../schema/content_performance_metrics.sql"),
        )
        .with_required_columns(vec!["id".into(), "content_id".into()]),
        SchemaDefinition::new(
            "campaign_links",
            include_str!("../schema/campaign_links.sql"),
        )
        .with_required_columns(vec!["id".into(), "short_code".into(), "target_url".into()]),
        SchemaDefinition::new("link_clicks", include_str!("../schema/link_clicks.sql"))
            .with_required_columns(vec!["id".into(), "link_id".into(), "clicked_at".into()]),
        SchemaDefinition::new("content_files", include_str!("../schema/content_files.sql"))
            .with_required_columns(vec!["id".into(), "content_id".into(), "file_id".into()]),
        SchemaDefinition::sql_only(include_str!("../schema/report_views.sql")),
    ]
}
