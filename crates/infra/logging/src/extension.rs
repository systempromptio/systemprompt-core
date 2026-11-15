//! `Extension` registration for the logging subsystem: schema, priority,
//! required flag.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct LoggingExtension;

impl Extension for LoggingExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "logging",
            name: "Logging",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for schema in schemas() {
            c.schema(schema);
        }
        c.migrations(extension_migrations!());
        Ok(())
    }
}

fn schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new("logs", include_str!("../schema/log.sql")).with_required_columns(
            vec![
                "id".into(),
                "timestamp".into(),
                "level".into(),
                "module".into(),
                "message".into(),
            ],
        ),
        SchemaDefinition::new("analytics_events", include_str!("../schema/analytics.sql"))
            .with_required_columns(vec![
                "id".into(),
                "user_id".into(),
                "event_type".into(),
                "event_category".into(),
                "severity".into(),
                "timestamp".into(),
            ]),
        SchemaDefinition::new(
            "log_rollup_hourly",
            include_str!("../schema/log_rollup_hourly.sql"),
        )
        .with_required_columns(vec![
            "bucket".into(),
            "level".into(),
            "module".into(),
            "log_count".into(),
        ]),
        SchemaDefinition::sql_only(include_str!("../schema/trace_facts_logs.sql")),
        SchemaDefinition::new(
            "analytics_link_click_rollup_hourly",
            include_str!("../schema/analytics_link_click_rollup_hourly.sql"),
        )
        .with_required_columns(vec!["bucket".into(), "endpoint".into(), "clicks".into()]),
        SchemaDefinition::sql_only(include_str!("../schema/analytics_event_link_facts.sql")),
    ]
}
