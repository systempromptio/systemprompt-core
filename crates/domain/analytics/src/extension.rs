//! Registers analytics-owned reporting, engagement, reputation, funnel and
//! feedback schemas, seeds and migrations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
                "analytics_ingestion_producers",
                include_str!("../schema/ingestion_producers.sql"),
            ),
            SchemaDefinition::new(
                "analytics_feedback_facts",
                include_str!("../schema/feedback_facts.sql"),
            ),
            SchemaDefinition::new(
                "analytics_feedback_snapshots",
                include_str!("../schema/feedback_snapshots.sql"),
            ),
            SchemaDefinition::new(
                "analytics_projection_state",
                include_str!("../schema/reporting.sql"),
            )
            .with_required_columns(vec!["generation".into(), "cutoff_revision".into()]),
            SchemaDefinition::sql_only(include_str!("../schema/reporting_privacy.sql")),
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
            SchemaDefinition::new("funnel_steps", include_str!("../schema/funnel_steps.sql"))
                .with_required_columns(vec!["funnel_id".into(), "step_order".into()]),
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

    fn seeds(&self) -> Vec<Seed> {
        vec![Seed::new(
            "analytics_projection_state",
            crate::projection::REPORTING_STATE_SEED,
        )]
    }
}

register_extension!(AnalyticsExtension);
