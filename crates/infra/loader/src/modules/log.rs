use std::path::PathBuf;
use systemprompt_models::modules::{ApiConfig, Module, ModuleSchema, SchemaSource};

pub fn define() -> Module {
    Module {
        uuid: uuid(),
        name: "log".into(),
        version: "0.0.1".into(),
        display_name: "System Logs".into(),
        description: Some(
            "Infrastructure module for system logging and monitoring with database persistence"
                .into(),
        ),
        weight: Some(-90),
        dependencies: vec!["users".into()],
        schemas: Some(vec![
            ModuleSchema {
                table: "logs".into(),
                sql: SchemaSource::Inline(include_str!("../../../logging/schema/log.sql").into()),
                required_columns: vec![
                    "id".into(),
                    "level".into(),
                    "module".into(),
                    "message".into(),
                    "metadata".into(),
                    "timestamp".into(),
                ],
            },
            ModuleSchema {
                table: "analytics_events".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../logging/schema/analytics.sql").into(),
                ),
                required_columns: vec![
                    "id".into(),
                    "event_type".into(),
                    "event_category".into(),
                    "timestamp".into(),
                ],
            },
        ]),
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: Some(ApiConfig {
            enabled: true,
            path_prefix: Some("/api/v1/log".into()),
            openapi_path: None,
        }),
        path: PathBuf::new(),
    }
}

fn uuid() -> String {
    "log-module-0001-0001-000000000001".into()
}
