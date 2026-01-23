use systemprompt_extension::prelude::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct McpExtension;

impl Extension for McpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "mcp",
            name: "MCP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn migration_weight(&self) -> u32 {
        25
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::inline(
            "mcp_tool_executions",
            include_str!("../schema/mcp_tool_executions.sql"),
        )
        .with_required_columns(vec![
            "id".into(),
            "tool_name".into(),
            "server_name".into(),
            "created_at".into(),
        ])]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }
}

register_extension!(McpExtension);
