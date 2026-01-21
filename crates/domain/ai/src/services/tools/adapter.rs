use systemprompt_identifiers::{AiToolCallId, McpServerId};
use systemprompt_models::ai::tools::{CallToolResult, McpTool, ToolCall};
use systemprompt_models::RequestContext;
use systemprompt_traits::{
    ToolCallRequest, ToolCallResult as TraitToolCallResult, ToolContent, ToolContext,
    ToolDefinition,
};

pub fn mcp_tool_to_definition(tool: &McpTool) -> ToolDefinition {
    ToolDefinition {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: tool.input_schema.clone(),
        output_schema: tool.output_schema.clone(),
        service_id: tool.service_id.to_string(),
        terminal_on_success: tool.terminal_on_success,
        model_config: tool
            .model_config
            .as_ref()
            .and_then(|c| serde_json::to_value(c).ok()),
    }
}

pub fn definition_to_mcp_tool(def: &ToolDefinition) -> McpTool {
    McpTool {
        name: def.name.clone(),
        description: def.description.clone(),
        input_schema: def.input_schema.clone(),
        output_schema: def.output_schema.clone(),
        service_id: McpServerId::new(def.service_id.clone()),
        terminal_on_success: def.terminal_on_success,
        model_config: def
            .model_config
            .as_ref()
            .and_then(|c| serde_json::from_value(c.clone()).ok()),
    }
}

pub fn tool_call_to_request(call: &ToolCall) -> ToolCallRequest {
    ToolCallRequest {
        tool_call_id: call.ai_tool_call_id.to_string(),
        name: call.name.clone(),
        arguments: call.arguments.clone(),
    }
}

pub fn request_to_tool_call(request: &ToolCallRequest) -> ToolCall {
    ToolCall {
        ai_tool_call_id: AiToolCallId::new(request.tool_call_id.clone()),
        name: request.name.clone(),
        arguments: request.arguments.clone(),
    }
}

pub fn rmcp_result_to_trait_result(result: &CallToolResult) -> TraitToolCallResult {
    let content = result
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

    TraitToolCallResult {
        content,
        structured_content: result.structured_content.clone(),
        is_error: result.is_error,
        meta: result
            .meta
            .as_ref()
            .and_then(|m| serde_json::to_value(m).ok()),
    }
}

pub fn trait_result_to_rmcp_result(result: &TraitToolCallResult) -> CallToolResult {
    use rmcp::model::{
        Annotated, Content, RawContent, RawImageContent, RawResource, RawTextContent,
    };

    let content: Vec<Content> = result
        .content
        .iter()
        .map(|c| match c {
            ToolContent::Text { text } => Annotated {
                raw: RawContent::Text(RawTextContent {
                    text: text.clone(),
                    meta: None,
                }),
                annotations: None,
            },
            ToolContent::Image { data, mime_type } => Annotated {
                raw: RawContent::Image(RawImageContent {
                    data: data.clone(),
                    mime_type: mime_type.clone(),
                    meta: None,
                }),
                annotations: None,
            },
            ToolContent::Resource { uri, mime_type } => Annotated {
                raw: RawContent::ResourceLink(RawResource {
                    uri: uri.clone(),
                    name: uri.clone(),
                    title: None,
                    description: None,
                    mime_type: mime_type.clone(),
                    size: None,
                    icons: None,
                    meta: None,
                }),
                annotations: None,
            },
        })
        .collect();

    CallToolResult {
        content,
        structured_content: result.structured_content.clone(),
        is_error: result.is_error,
        meta: result
            .meta
            .as_ref()
            .and_then(|m| serde_json::from_value(m.clone()).ok()),
    }
}

pub fn request_context_to_tool_context(ctx: &RequestContext) -> ToolContext {
    let mut tool_ctx = ToolContext::new(ctx.auth_token().as_str())
        .with_session_id(ctx.session_id().to_string())
        .with_trace_id(ctx.trace_id().to_string())
        .with_header("x-context-id", ctx.context_id().as_str())
        .with_header("x-user-id", ctx.user_id().as_str())
        .with_header("x-agent-name", ctx.agent_name().as_str());

    if let Some(ai_tool_call_id) = ctx.ai_tool_call_id() {
        tool_ctx = tool_ctx.with_ai_tool_call_id(ai_tool_call_id.to_string());
    }

    if let Some(task_id) = ctx.task_id() {
        tool_ctx = tool_ctx.with_header("x-task-id", task_id.as_str());
    }

    tool_ctx
}
