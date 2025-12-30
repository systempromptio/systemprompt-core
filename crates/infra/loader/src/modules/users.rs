use std::path::PathBuf;
use systemprompt_models::modules::{ApiConfig, Module, ModuleSchema, SchemaSource};

pub fn define() -> Module {
    Module {
        uuid: uuid(),
        name: "users".into(),
        version: "0.0.1".into(),
        display_name: "Core Users".into(),
        description: Some("Foundation user management, sessions, and authentication".into()),
        weight: Some(1),
        dependencies: vec![],
        schemas: Some(vec![
            ModuleSchema {
                table: "users".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/users/schema/users.sql").into(),
                ),
                required_columns: vec![
                    "id".into(),
                    "name".into(),
                    "email".into(),
                    "created_at".into(),
                ],
            },
            ModuleSchema {
                table: "user_sessions".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/users/schema/user_sessions.sql").into(),
                ),
                required_columns: vec!["session_id".into(), "user_id".into(), "started_at".into()],
            },
            ModuleSchema {
                table: "banned_ips".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/users/schema/banned_ips.sql").into(),
                ),
                required_columns: vec!["ip_address".into(), "reason".into(), "banned_at".into()],
            },
            ModuleSchema {
                table: String::new(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/users/schema/session_analytics_views.sql")
                        .into(),
                ),
                required_columns: vec![],
            },
            ModuleSchema {
                table: String::new(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/users/schema/referrer_analytics_views.sql")
                        .into(),
                ),
                required_columns: vec![],
            },
            ModuleSchema {
                table: String::new(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/users/schema/bot_analytics_views.sql").into(),
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
            path_prefix: Some("/api/v1/users".into()),
            openapi_path: None,
        }),
        path: PathBuf::new(),
    }
}

fn uuid() -> String {
    "users-module-0001-0001-000000000001".into()
}
