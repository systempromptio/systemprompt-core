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

    fn migration_weight(&self) -> u32 {
        55
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::inline(
            "scheduled_jobs",
            include_str!("../schema/scheduled_jobs.sql"),
        )
        .with_required_columns(vec![
            "id".into(),
            "job_type".into(),
            "status".into(),
            "created_at".into(),
        ])]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }
}

register_extension!(SchedulerExtension);
