//! Extension-framework registration for the scheduler crate.
//!
//! [`SchedulerExtension`] declares the schema for the `scheduled_jobs` table
//! and registers itself with the platform extension registry via
//! [`register_extension!`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

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

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::new(
                "scheduled_jobs",
                include_str!("../schema/scheduled_jobs.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "job_name".into(),
                "created_at".into(),
            ]),
            SchemaDefinition::new(
                "config_plane_state",
                include_str!("../schema/config_plane_state.sql"),
            )
            .with_required_columns(vec![
                "plane".into(),
                "authority".into(),
                "outcome".into(),
            ]),
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
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }
}

register_extension!(SchedulerExtension);
