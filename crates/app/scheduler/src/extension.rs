//! The scheduler member: the `scheduled_jobs`, config-plane, OTLP export and
//! licence check-in schemas and migrations, and the kernel's built-in jobs
//! (config reconcile, licence check-in, the inventory refresh, the drainers of
//! kernel-owned projections, session housekeeping, database cleanup, Vertex
//! discovery and, with `otel`, the OTLP export).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

use crate::jobs::{
    BackfillRollupsJob, CleanupInactiveSessionsJob, ConfigReconcileJob, DatabaseCleanupJob,
    InventoryRefreshJob, LicenceCheckinJob, LogSummaryRollupJob, OutboxBacklogRollupJob,
    SessionBotTypeRollupJob, TraceFactsRollupJob, VertexDiscoveryJob,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct SchedulerExtension;

impl Extension for SchedulerExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "scheduler",
            name: "Scheduler",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for schema in schemas() {
            c.schema(schema);
        }
        c.migrations(extension_migrations!())
            .job(ConfigReconcileJob)
            .job(LicenceCheckinJob)
            .job(InventoryRefreshJob)
            .job(BackfillRollupsJob)
            .job(LogSummaryRollupJob)
            .job(OutboxBacklogRollupJob)
            .job(SessionBotTypeRollupJob)
            .job(TraceFactsRollupJob)
            .job(CleanupInactiveSessionsJob)
            .job(DatabaseCleanupJob)
            .job(VertexDiscoveryJob);
        crate::jobs::database_cleanup::kernel_purges::contribute(c);
        #[cfg(feature = "otel")]
        c.job(crate::jobs::OtlpExportJob);
        Ok(())
    }
}

fn schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "scheduled_jobs",
            include_str!("../schema/scheduled_jobs.sql"),
        )
        .with_required_columns(vec!["id".into(), "job_name".into(), "created_at".into()]),
        SchemaDefinition::new(
            "config_plane_state",
            include_str!("../schema/config_plane_state.sql"),
        )
        .with_required_columns(vec!["plane".into(), "authority".into(), "outcome".into()]),
        SchemaDefinition::new(
            "config_change_audit",
            include_str!("../schema/config_change_audit.sql"),
        )
        .with_required_columns(vec![
            "id".into(),
            "plane".into(),
            "entity".into(),
            "before_value".into(),
            "after_value".into(),
        ]),
        SchemaDefinition::new(
            "config_plane_base",
            include_str!("../schema/config_plane_base.sql"),
        )
        .with_required_columns(vec![
            "plane".into(),
            "entity".into(),
            "declared_hash".into(),
            "source".into(),
        ]),
        SchemaDefinition::new(
            "config_conflicts",
            include_str!("../schema/config_conflicts.sql"),
        )
        .with_required_columns(vec![
            "id".into(),
            "plane".into(),
            "entity".into(),
            "file_hash".into(),
            "db_hash".into(),
        ]),
        SchemaDefinition::new(
            "otlp_export_state",
            include_str!("../schema/otlp_export_state.sql"),
        )
        .with_required_columns(vec![
            "signal".into(),
            "watermark".into(),
            "watermark_id".into(),
            "batches_total".into(),
        ]),
        SchemaDefinition::new(
            "licence_checkin_state",
            include_str!("../schema/licence_checkin_state.sql"),
        )
        .with_required_columns(vec![
            "id".into(),
            "last_attempt_at".into(),
            "last_success_at".into(),
            "last_error".into(),
        ]),
    ]
}
