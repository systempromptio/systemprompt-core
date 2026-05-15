//! Extension-framework registration for the scheduler crate.
//!
//! [`SchedulerExtension`] declares the schema for the `scheduled_jobs` table
//! and registers itself with the platform extension registry via
//! [`register_extension!`].

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
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }
}

register_extension!(SchedulerExtension);
