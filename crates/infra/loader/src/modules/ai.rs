use std::path::PathBuf;
use systemprompt_models::modules::{ApiConfig, Module, ModuleSchema, SchemaSource};

pub fn define() -> Module {
    Module {
        uuid: uuid(),
        name: "ai".into(),
        version: "0.0.1".into(),
        display_name: "AI Service".into(),
        description: Some(
            "Artificial Intelligence service for natural language processing and intelligent \
             automation"
                .into(),
        ),
        weight: Some(60),
        dependencies: vec![],
        schemas: Some(vec![
            ModuleSchema {
                table: "ai_requests".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/ai/schema/ai_requests.sql").into(),
                ),
                required_columns: vec![
                    "id".into(),
                    "request_id".into(),
                    "provider".into(),
                    "model".into(),
                ],
            },
            ModuleSchema {
                table: "ai_request_messages".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/ai/schema/ai_request_messages.sql").into(),
                ),
                required_columns: vec![
                    "id".into(),
                    "request_id".into(),
                    "role".into(),
                    "content".into(),
                ],
            },
            ModuleSchema {
                table: "ai_request_tool_calls".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/ai/schema/ai_request_tool_calls.sql").into(),
                ),
                required_columns: vec![
                    "id".into(),
                    "request_id".into(),
                    "tool_name".into(),
                    "tool_input".into(),
                    "mcp_execution_id".into(),
                ],
            },
        ]),
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: Some(ApiConfig {
            enabled: true,
            path_prefix: Some("/api/v1/ai".into()),
            openapi_path: Some("/api/v1/ai/docs".into()),
        }),
        path: PathBuf::new(),
    }
}

fn uuid() -> String {
    "ai-module-0001-0001-000000000001".into()
}
