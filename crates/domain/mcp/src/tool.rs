use crate::error::McpError;
use crate::response::McpResponseBuilder;
use crate::schema::McpOutputSchema;
use async_trait::async_trait;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value as JsonValue;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_models::RequestContext;

#[async_trait]
pub trait McpToolHandler: Send + Sync {
    type Input: DeserializeOwned + JsonSchema + Send;
    type Output: Serialize + JsonSchema + McpOutputSchema + Send;

    fn tool_name(&self) -> &'static str;

    fn description(&self) -> &'static str {
        ""
    }

    fn input_schema(&self) -> JsonValue {
        let schema = schemars::schema_for!(Self::Input);
        serde_json::to_value(&schema).unwrap_or(JsonValue::Null)
    }

    fn output_schema(&self) -> JsonValue {
        Self::Output::validated_schema()
    }

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError>;
}

pub async fn call_tool<H: McpToolHandler>(
    handler: &H,
    request: &CallToolRequestParam,
    ctx: &RequestContext,
) -> Result<CallToolResult, McpError> {
    let exec_id = McpExecutionId::generate();

    let input: H::Input = parse_input(request)?;

    let (output, summary) = handler.handle(input, ctx, &exec_id).await?;

    McpResponseBuilder::new(output, handler.tool_name(), ctx, &exec_id).build(summary)
}

fn parse_input<T: DeserializeOwned>(request: &CallToolRequestParam) -> Result<T, McpError> {
    let args_value = request
        .arguments
        .as_ref()
        .map(|m| JsonValue::Object(m.clone()))
        .unwrap_or(JsonValue::Object(serde_json::Map::new()));

    serde_json::from_value(args_value).map_err(|e| {
        tracing::warn!(
            error = %e,
            tool = %request.name,
            "Failed to parse tool input"
        );
        McpError::SchemaValidation(format!("Invalid tool input: {e}"))
    })
}
