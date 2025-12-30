use std::path::PathBuf;
use systemprompt_models::modules::{ApiConfig, Module, ModuleSchema, SchemaSource};

pub fn define() -> Module {
    Module {
        uuid: uuid(),
        name: "agent".into(),
        version: "0.0.1".into(),
        display_name: "Core Agent Protocol".into(),
        description: Some(
            "Agent protocol implementation for orchestrating and managing AI agents".into(),
        ),
        weight: Some(25),
        dependencies: vec!["users".into(), "oauth".into(), "mcp".into()],
        schemas: Some(vec![
            ModuleSchema {
                table: "user_contexts".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/agent/schema/user_contexts.sql").into(),
                ),
                required_columns: vec!["id".into(), "user_id".into()],
            },
            ModuleSchema {
                table: "agent_tasks".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/agent/schema/agent_tasks.sql").into(),
                ),
                required_columns: vec!["id".into(), "uuid".into()],
            },
            ModuleSchema {
                table: "task_messages".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/agent/schema/task_messages.sql").into(),
                ),
                required_columns: vec!["id".into(), "task_id".into()],
            },
            ModuleSchema {
                table: "message_parts".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/agent/schema/message_parts.sql").into(),
                ),
                required_columns: vec!["id".into(), "message_id".into()],
            },
            ModuleSchema {
                table: "task_artifacts".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/agent/schema/task_artifacts.sql").into(),
                ),
                required_columns: vec!["id".into(), "task_id".into()],
            },
            ModuleSchema {
                table: "artifact_parts".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/agent/schema/artifact_parts.sql").into(),
                ),
                required_columns: vec!["id".into(), "artifact_id".into()],
            },
            ModuleSchema {
                table: "context_agents".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/agent/schema/context_agents.sql").into(),
                ),
                required_columns: vec!["id".into(), "context_id".into(), "agent_id".into()],
            },
            ModuleSchema {
                table: "context_notifications".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/agent/schema/context_notifications.sql")
                        .into(),
                ),
                required_columns: vec!["id".into(), "context_id".into(), "agent_id".into()],
            },
            ModuleSchema {
                table: "task_push_notification_configs".into(),
                sql: SchemaSource::Inline(
                    include_str!(
                        "../../../../domain/agent/schema/task_push_notification_configs.sql"
                    )
                    .into(),
                ),
                required_columns: vec![
                    "id".into(),
                    "task_id".into(),
                    "url".into(),
                    "endpoint".into(),
                ],
            },
            ModuleSchema {
                table: "agent_skills".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/agent/schema/agent_skills.sql").into(),
                ),
                required_columns: vec![
                    "skill_id".into(),
                    "file_path".into(),
                    "name".into(),
                    "instructions".into(),
                ],
            },
            ModuleSchema {
                table: "task_execution_steps".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/agent/schema/task_execution_steps.sql").into(),
                ),
                required_columns: vec![
                    "id".into(),
                    "step_id".into(),
                    "task_id".into(),
                    "step_type".into(),
                    "status".into(),
                ],
            },
            ModuleSchema {
                table: "services".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/agent/schema/services.sql").into(),
                ),
                required_columns: vec![
                    "name".into(),
                    "module_name".into(),
                    "port".into(),
                    "status".into(),
                ],
            },
            ModuleSchema {
                table: String::new(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/agent/schema/user_session_analytics.sql")
                        .into(),
                ),
                required_columns: vec![],
            },
        ]),
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: Some(ApiConfig {
            enabled: true,
            path_prefix: Some("/api/v1/agents".into()),
            openapi_path: Some("/api/v1/agents/docs".into()),
        }),
        path: PathBuf::new(),
    }
}

fn uuid() -> String {
    "agent-module-0001-0001-000000000001".into()
}
