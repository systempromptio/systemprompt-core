//! Extension registration — wires analytics schemas (sessions, funnels,
//! engagement events, fingerprint reputation, anomaly thresholds) and
//! schema-evolution migrations into the extension framework.

use systemprompt_extension::prelude::*;

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

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::new(
                "engagement_events",
                include_str!("../schema/engagement_events.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "session_id".into(),
                "created_at".into(),
            ]),
            SchemaDefinition::new(
                "anomaly_thresholds",
                include_str!("../schema/anomaly_thresholds.sql"),
            )
            .with_required_columns(vec!["metric_name".into()]),
            SchemaDefinition::new(
                "fingerprint_reputation",
                include_str!("../schema/fingerprint_reputation.sql"),
            )
            .with_required_columns(vec!["fingerprint_hash".into()]),
            SchemaDefinition::new("funnels", include_str!("../schema/funnels.sql"))
                .with_required_columns(vec!["id".into(), "name".into()]),
            SchemaDefinition::new(
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
        extension_migrations!()
    }
}

register_extension!(AnalyticsExtension);
