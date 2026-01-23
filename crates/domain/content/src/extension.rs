use systemprompt_extension::prelude::*;

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

    fn migration_weight(&self) -> u32 {
        45
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::inline(
                "markdown_categories",
                include_str!("../schema/markdown_categories.sql"),
            )
            .with_required_columns(vec!["id".into(), "name".into(), "slug".into()]),
            SchemaDefinition::inline(
                "markdown_content",
                include_str!("../schema/markdown_content.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "slug".into(),
                "title".into(),
                "created_at".into(),
            ]),
            SchemaDefinition::inline("markdown_fts", include_str!("../schema/markdown_fts.sql")),
            SchemaDefinition::inline(
                "content_performance_metrics",
                include_str!("../schema/content_performance_metrics.sql"),
            )
            .with_required_columns(vec!["id".into(), "content_id".into()]),
            SchemaDefinition::inline(
                "campaign_links",
                include_str!("../schema/campaign_links.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "short_code".into(),
                "target_url".into(),
            ]),
            SchemaDefinition::inline("link_clicks", include_str!("../schema/link_clicks.sql"))
                .with_required_columns(vec!["id".into(), "link_id".into(), "clicked_at".into()]),
            SchemaDefinition::inline(
                "link_analytics_views",
                include_str!("../schema/link_analytics_views.sql"),
            ),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users", "analytics"]
    }
}

register_extension!(ContentExtension);
