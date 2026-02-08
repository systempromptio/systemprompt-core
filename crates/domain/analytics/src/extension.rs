use systemprompt_extension::prelude::*;

const MIGRATION_001_EVENT_TYPE: &str = r"
ALTER TABLE engagement_events ADD COLUMN IF NOT EXISTS event_type VARCHAR(50) NOT NULL DEFAULT 'page_exit';
CREATE INDEX IF NOT EXISTS idx_engagement_events_event_type ON engagement_events(event_type);
";

#[derive(Debug, Clone, Copy, Default)]
pub struct AnalyticsExtension;

impl Extension for AnalyticsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "analytics",
            name: "Analytics",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn migration_weight(&self) -> u32 {
        20
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::inline(
                "engagement_events",
                include_str!("../schema/engagement_events.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "session_id".into(),
                "created_at".into(),
            ]),
            SchemaDefinition::inline(
                "anomaly_thresholds",
                include_str!("../schema/anomaly_thresholds.sql"),
            )
            .with_required_columns(vec!["metric_name".into()]),
            SchemaDefinition::inline(
                "fingerprint_reputation",
                include_str!("../schema/fingerprint_reputation.sql"),
            )
            .with_required_columns(vec!["fingerprint_hash".into()]),
            SchemaDefinition::inline("funnels", include_str!("../schema/funnels.sql"))
                .with_required_columns(vec!["id".into(), "name".into()]),
            SchemaDefinition::inline(
                "funnel_progress",
                include_str!("../schema/funnel_progress.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "funnel_id".into(),
                "session_id".into(),
            ]),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration::new(
            1,
            "add_engagement_event_type",
            MIGRATION_001_EVENT_TYPE,
        )]
    }
}

register_extension!(AnalyticsExtension);
