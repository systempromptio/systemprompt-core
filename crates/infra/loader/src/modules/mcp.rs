use std::path::PathBuf;
use systemprompt_models::modules::{ApiConfig, Module, ModuleSchema, SchemaSource};

pub fn define() -> Module {
    Module {
        uuid: uuid(),
        name: "mcp".into(),
        version: "0.0.1".into(),
        display_name: "MCP Service Manager".into(),
        description: Some("Model Context Protocol server orchestration and management".into()),
        weight: Some(1),
        dependencies: vec![],
        schemas: Some(vec![ModuleSchema {
            table: "mcp_tool_executions".into(),
            sql: SchemaSource::Inline(
                include_str!("../../../../domain/mcp/schema/mcp_tool_executions.sql").into(),
            ),
            required_columns: vec![
                "mcp_execution_id".into(),
                "tool_name".into(),
                "mcp_server_name".into(),
                "started_at".into(),
                "status".into(),
            ],
        }]),
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: Some(ApiConfig {
            enabled: true,
            path_prefix: Some("/api/v1/mcp".into()),
            openapi_path: None,
        }),
        path: PathBuf::new(),
    }
}

fn uuid() -> String {
    "mcp-module-0001-0001-000000000001".into()
}
