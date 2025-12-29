use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use systemprompt_identifiers::AiToolCallId;
use systemprompt_models::{McpTool, RequestContext, ToolCall};

use super::plan_executor::ToolExecutorTrait;
use super::ExecutionContext;

#[derive(Debug)]
pub struct ContextToolExecutor {
    pub context: ExecutionContext,
}

#[async_trait]
impl ToolExecutorTrait for ContextToolExecutor {
    async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Value,
        tools: &[McpTool],
        ctx: &RequestContext,
    ) -> Result<Value> {
        let tool_call = ToolCall {
            ai_tool_call_id: AiToolCallId::new(format!("call_{}", tool_name)),
            name: tool_name.to_string(),
            arguments,
        };

        let (_, results) = self
            .context
            .ai_service
            .execute_tools(
                vec![tool_call],
                tools,
                ctx,
                Some(&self.context.agent_runtime.tool_model_overrides),
            )
            .await;

        let result = results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Tool {} returned no result", tool_name))?;

        if result.is_error.unwrap_or(false) {
            let error_msg = result
                .content
                .into_iter()
                .next()
                .and_then(|c| {
                    if let rmcp::model::RawContent::Text(text_content) = c.raw {
                        Some(text_content.text)
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Tool {} failed: {}", tool_name, error_msg));
        }

        result
            .content
            .into_iter()
            .next()
            .and_then(|c| {
                if let rmcp::model::RawContent::Text(text_content) = c.raw {
                    let text = text_content.text;
                    serde_json::from_str(&text)
                        .ok()
                        .or(Some(Value::String(text)))
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Tool {} returned empty or non-text content", tool_name))
    }
}
