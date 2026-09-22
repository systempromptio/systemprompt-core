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
        let mut schemas = capture_schemas();
        schemas.extend(fact_schemas());
        schemas.extend(snapshot_schemas());
        schemas.extend(projection_schemas());
        schemas.extend(behavioral_schemas());
        schemas
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

fn capture_schemas() -> Vec<SchemaDefinition> {
    vec![SchemaDefinition::sql_only(include_str!(
        "../schema/reporting_privacy.sql"
    ))]
}

fn fact_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "analytics_ingestion_producers",
            include_str!("../schema/ingestion_producers.sql"),
        ),
        SchemaDefinition::new(
            "analytics_fact_changes",
            include_str!("../schema/analytics_fact_changes.sql"),
        ),
        SchemaDefinition::new(
            "analytics_normalized_facts",
            include_str!("../schema/analytics_normalized_facts.sql"),
        ),
        SchemaDefinition::new(
            "analytics_fact_checkpoints",
            include_str!("../schema/analytics_fact_checkpoints.sql"),
        ),
        SchemaDefinition::new(
            "analytics_fact_deltas",
            include_str!("../schema/analytics_fact_deltas.sql"),
        ),
        SchemaDefinition::new(
            "analytics_fact_backfills",
            include_str!("../schema/analytics_fact_backfills.sql"),
        ),
        SchemaDefinition::new(
            "analytics_fact_backfill_pages",
            include_str!("../schema/analytics_fact_backfill_pages.sql"),
        ),
        SchemaDefinition::new(
            "analytics_fact_consumers",
            include_str!("../schema/analytics_fact_consumers.sql"),
        ),
    ]
}

fn snapshot_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "analytics_snapshot_dirty",
            include_str!("../schema/analytics_snapshot_dirty.sql"),
        ),
        SchemaDefinition::new(
            "analytics_snapshot_shadow",
            include_str!("../schema/analytics_snapshot_shadow.sql"),
        ),
        SchemaDefinition::new(
            "analytics_snapshot_daily",
            include_str!("../schema/analytics_snapshot_daily.sql"),
        ),
        SchemaDefinition::new(
            "analytics_snapshot_identities",
            include_str!("../schema/analytics_snapshot_identities.sql"),
        ),
        SchemaDefinition::new(
            "analytics_snapshot_state",
            include_str!("../schema/analytics_snapshot_state.sql"),
        ),
        SchemaDefinition::new(
            "analytics_snapshot_jobs",
            include_str!("../schema/analytics_snapshot_jobs.sql"),
        ),
    ]
}

fn projection_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "analytics_projection_state",
            include_str!("../schema/analytics_projection_state.sql"),
        )
        .with_required_columns(vec!["generation".into(), "cutoff_revision".into()]),
        SchemaDefinition::new(
            "analytics_projection_revisions",
            include_str!("../schema/analytics_projection_revisions.sql"),
        ),
        SchemaDefinition::new(
            "analytics_report_users",
            include_str!("../schema/analytics_report_users.sql"),
        ),
        SchemaDefinition::new(
            "analytics_report_user_sessions",
            include_str!("../schema/analytics_report_user_sessions.sql"),
        ),
        SchemaDefinition::new(
            "analytics_report_agent_tasks",
            include_str!("../schema/analytics_report_agent_tasks.sql"),
        ),
        SchemaDefinition::new(
            "analytics_report_task_messages",
            include_str!("../schema/analytics_report_task_messages.sql"),
        ),
        SchemaDefinition::new(
            "analytics_report_user_contexts",
            include_str!("../schema/analytics_report_user_contexts.sql"),
        ),
        SchemaDefinition::new(
            "analytics_report_ai_requests",
            include_str!("../schema/analytics_report_ai_requests.sql"),
        ),
        SchemaDefinition::new(
            "analytics_report_mcp_tool_executions",
            include_str!("../schema/analytics_report_mcp_tool_executions.sql"),
        ),
        SchemaDefinition::new(
            "analytics_report_markdown_content",
            include_str!("../schema/analytics_report_markdown_content.sql"),
        ),
        SchemaDefinition::new(
            "analytics_report_analytics_events",
            include_str!("../schema/analytics_report_analytics_events.sql"),
        ),
    ]
}

fn behavioral_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "analytics_feedback_snapshots",
            include_str!("../schema/analytics_feedback_snapshots.sql"),
        ),
        SchemaDefinition::new(
            "engagement_events",
            include_str!("../schema/engagement_events.sql"),
        )
        .with_required_columns(vec!["id".into(), "session_id".into(), "created_at".into()]),
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
        .with_required_columns(vec!["id".into(), "funnel_id".into(), "session_id".into()]),
    ]
}
