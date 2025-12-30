use std::path::PathBuf;
use systemprompt_models::modules::{ApiConfig, Module, ModuleSchema, SchemaSource};

pub fn define() -> Module {
    Module {
        uuid: uuid(),
        name: "content".into(),
        version: "0.0.1".into(),
        display_name: "Blog & Content Management".into(),
        description: Some(
            "Blog content management with markdown support and full-text search".into(),
        ),
        weight: Some(30),
        dependencies: vec!["users".into(), "oauth".into()],
        schemas: Some(vec![
            ModuleSchema {
                table: "markdown_categories".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/content/schema/markdown_categories.sql")
                        .into(),
                ),
                required_columns: vec!["id".into(), "name".into()],
            },
            ModuleSchema {
                table: "markdown_content".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/content/schema/markdown_content.sql").into(),
                ),
                required_columns: vec!["id".into(), "slug".into(), "title".into()],
            },
            ModuleSchema {
                table: "markdown_fts".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/content/schema/markdown_fts.sql").into(),
                ),
                required_columns: vec![],
            },
            ModuleSchema {
                table: "content_performance_metrics".into(),
                sql: SchemaSource::Inline(
                    include_str!(
                        "../../../../domain/content/schema/content_performance_metrics.sql"
                    )
                    .into(),
                ),
                required_columns: vec!["id".into(), "content_id".into()],
            },
            ModuleSchema {
                table: "campaign_links".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/content/schema/campaign_links.sql").into(),
                ),
                required_columns: vec!["id".into(), "short_code".into()],
            },
            ModuleSchema {
                table: "link_clicks".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/content/schema/link_clicks.sql").into(),
                ),
                required_columns: vec!["id".into(), "link_id".into()],
            },
            ModuleSchema {
                table: String::new(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/content/schema/link_analytics_views.sql")
                        .into(),
                ),
                required_columns: vec![],
            },
        ]),
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: Some(ApiConfig {
            enabled: true,
            path_prefix: Some("/api/v1/rag".into()),
            openapi_path: None,
        }),
        path: PathBuf::new(),
    }
}

fn uuid() -> String {
    "content-module-0001-0001-000000000001".into()
}
