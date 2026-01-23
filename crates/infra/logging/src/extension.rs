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

    fn migration_weight(&self) -> u32 {
        15
    }

    fn is_required(&self) -> bool {
        true
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::inline("logs", include_str!("../schema/log.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "timestamp".into(),
                    "level".into(),
                    "module".into(),
                    "message".into(),
                ]),
            SchemaDefinition::inline("analytics_events", include_str!("../schema/analytics.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "user_id".into(),
                    "event_type".into(),
                    "event_category".into(),
                    "severity".into(),
                    "timestamp".into(),
                ]),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["database", "users"]
    }
}

register_extension!(LoggingExtension);
