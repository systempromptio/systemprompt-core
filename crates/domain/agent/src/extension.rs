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

    fn migration_weight(&self) -> u32 {
        25
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::inline(
                "user_contexts",
                include_str!("../schema/user_contexts.sql"),
            )
            .with_required_columns(vec!["id".into(), "user_id".into(), "created_at".into()]),
            SchemaDefinition::inline("agent_tasks", include_str!("../schema/agent_tasks.sql"))
                .with_required_columns(vec![
                    "task_id".into(),
                    "context_id".into(),
                    "status".into(),
                    "created_at".into(),
                ]),
            SchemaDefinition::inline("task_messages", include_str!("../schema/task_messages.sql"))
                .with_required_columns(vec![
                    "message_id".into(),
                    "task_id".into(),
                    "role".into(),
                    "created_at".into(),
                ]),
            SchemaDefinition::inline("message_parts", include_str!("../schema/message_parts.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "message_id".into(),
                    "part_type".into(),
                ]),
            SchemaDefinition::inline(
                "task_artifacts",
                include_str!("../schema/task_artifacts.sql"),
            )
            .with_required_columns(vec![
                "artifact_id".into(),
                "task_id".into(),
                "created_at".into(),
            ]),
            SchemaDefinition::inline(
                "artifact_parts",
                include_str!("../schema/artifact_parts.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "artifact_id".into(),
                "part_type".into(),
            ]),
            SchemaDefinition::inline(
                "context_agents",
                include_str!("../schema/context_agents.sql"),
            )
            .with_required_columns(vec!["id".into(), "context_id".into(), "agent_id".into()]),
            SchemaDefinition::inline(
                "context_notifications",
                include_str!("../schema/context_notifications.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "context_id".into(),
                "notification_type".into(),
            ]),
            SchemaDefinition::inline(
                "task_push_notification_configs",
                include_str!("../schema/task_push_notification_configs.sql"),
            )
            .with_required_columns(vec!["id".into(), "task_id".into()]),
            SchemaDefinition::inline("agent_skills", include_str!("../schema/agent_skills.sql"))
                .with_required_columns(vec!["id".into(), "agent_id".into(), "skill_id".into()]),
            SchemaDefinition::inline(
                "task_execution_steps",
                include_str!("../schema/task_execution_steps.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "task_id".into(),
                "step_type".into(),
                "created_at".into(),
            ]),
            SchemaDefinition::inline("services", include_str!("../schema/services.sql"))
                .with_required_columns(vec!["id".into(), "name".into(), "service_type".into()]),
            SchemaDefinition::inline(
                "user_session_analytics",
                include_str!("../schema/user_session_analytics.sql"),
            ),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users", "oauth", "mcp"]
    }
}

register_extension!(AgentExtension);
