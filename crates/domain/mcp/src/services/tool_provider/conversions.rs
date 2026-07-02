use systemprompt_models::ai::tools::McpTool;
use systemprompt_traits::{ToolCallResult, ToolContent, ToolDefinition};

pub fn to_tool_definition(mcp_tool: &McpTool) -> ToolDefinition {
    ToolDefinition {
        name: mcp_tool.name.clone(),
        description: mcp_tool.description.clone(),
        input_schema: mcp_tool.input_schema.clone(),
        output_schema: mcp_tool.output_schema.clone(),
        service_id: mcp_tool.service_id.to_string(),
        terminal_on_success: mcp_tool.terminal_on_success,
        model_config: mcp_tool
            .model_config
            .as_ref()
            .and_then(|c| serde_json::to_value(c).ok()),
    }
}

pub fn to_tool_result(rmcp_result: &rmcp::model::CallToolResult) -> ToolCallResult {
    let content = rmcp_result
        .content
        .iter()
        .filter_map(|c| match &c.raw {
            rmcp::model::RawContent::Text(text) => Some(ToolContent::Text {
                text: text.text.clone(),
            }),
            rmcp::model::RawContent::Image(img) => Some(ToolContent::Image {
                data: img.data.clone(),
                mime_type: img.mime_type.clone(),
            }),
            rmcp::model::RawContent::ResourceLink(res) => Some(ToolContent::Resource {
                uri: res.uri.clone(),
                mime_type: res.mime_type.clone(),
            }),
            _ => None,
        })
        .collect();

    ToolCallResult {
        content,
        structured_content: rmcp_result.structured_content.clone(),
        is_error: rmcp_result.is_error,
        meta: rmcp_result
            .meta
            .as_ref()
            .and_then(|m| serde_json::to_value(m).ok()),
    }
}
