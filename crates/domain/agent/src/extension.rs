//! `Extension` registration for the agent domain: schemas, migrations,
//! dependencies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

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

    fn schemas(&self) -> Vec<SchemaDefinition> {
        let mut schemas = conversation_schemas();
        schemas.extend(artifact_schemas());
        schemas.extend(context_schemas());
        schemas.extend(task_tracking_schemas());
        schemas.extend(service_schemas());
        schemas
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users", "oauth", "mcp", "ai"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }

    fn cross_extension_tables(&self) -> Vec<&'static str> {
        vec!["ai_requests"]
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
            "task_push_notification_configs",
            include_str!("../schema/task_push_notification_configs.sql"),
        )
        .with_required_columns(vec!["id".into(), "task_id".into()]),
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

fn service_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new("services", include_str!("../schema/services.sql"))
            .with_required_columns(vec!["name".into(), "module_name".into(), "status".into()]),
        SchemaDefinition::new(
            "user_session_analytics",
            include_str!("../schema/user_session_analytics.sql"),
        ),
    ]
}

register_extension!(AgentExtension);
