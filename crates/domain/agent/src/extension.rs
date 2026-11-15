//! [`AgentExtension`]: contributes the agent tables, migrations and jobs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

use crate::jobs::{CleanupEmptyContextsJob, UserContextCountersRollupJob};

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentExtension;

impl Extension for AgentExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "agent",
            name: "Agent",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for schema in conversation_schemas()
            .into_iter()
            .chain(artifact_schemas())
            .chain(context_schemas())
            .chain(task_tracking_schemas())
        {
            c.schema(schema);
        }
        c.schema(SchemaDefinition::sql_only(include_str!(
            "../schema/user_context_counters.sql"
        )))
        .schema(SchemaDefinition::sql_only(include_str!(
            "../schema/trace_facts_agent.sql"
        )))
        .migrations(extension_migrations!())
        .cross_table("ai_requests")
        .cross_table("services")
        .job(CleanupEmptyContextsJob)
        .job(UserContextCountersRollupJob);
        Ok(())
    }
}

fn conversation_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new("user_contexts", include_str!("../schema/user_contexts.sql"))
            .with_required_columns(vec![
                "context_id".into(),
                "user_id".into(),
                "created_at".into(),
            ]),
        SchemaDefinition::new("agent_tasks", include_str!("../schema/agent_tasks.sql"))
            .with_required_columns(vec![
                "task_id".into(),
                "context_id".into(),
                "status".into(),
                "created_at".into(),
            ]),
        SchemaDefinition::new("task_messages", include_str!("../schema/task_messages.sql"))
            .with_required_columns(vec![
                "id".into(),
                "task_id".into(),
                "role".into(),
                "created_at".into(),
            ]),
        SchemaDefinition::new("message_parts", include_str!("../schema/message_parts.sql"))
            .with_required_columns(vec!["id".into(), "message_id".into(), "part_kind".into()]),
    ]
}

fn context_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "context_agents",
            include_str!("../schema/context_agents.sql"),
        )
        .with_required_columns(vec!["id".into(), "context_id".into(), "agent_name".into()]),
        SchemaDefinition::new(
            "context_notifications",
            include_str!("../schema/context_notifications.sql"),
        )
        .with_required_columns(vec![
            "id".into(),
            "context_id".into(),
            "notification_type".into(),
        ]),
    ]
}

fn task_tracking_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "task_execution_steps",
            include_str!("../schema/task_execution_steps.sql"),
        )
        .with_required_columns(vec![
            "step_id".into(),
            "task_id".into(),
            "step_type".into(),
        ]),
    ]
}

fn artifact_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "task_artifacts",
            include_str!("../schema/task_artifacts.sql"),
        )
        .with_required_columns(vec!["id".into(), "task_id".into(), "artifact_id".into()]),
        SchemaDefinition::new(
            "artifact_parts",
            include_str!("../schema/artifact_parts.sql"),
        )
        .with_required_columns(vec!["id".into(), "artifact_id".into(), "part_kind".into()]),
    ]
}
