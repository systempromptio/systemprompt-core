use crate::services::shared::{AgentServiceError, Result};
use async_trait::async_trait;
use serde_json::Value;
use systemprompt_identifiers::AiToolCallId;
use systemprompt_models::{McpTool, RequestContext, ToolCall};

use super::ExecutionContext;
use super::plan_executor::ToolExecutorTrait;

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
            .ok_or_else(|| AgentServiceError::Internal(format!("Tool {} returned no result", tool_name)))?;

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
            return Err(AgentServiceError::Internal(format!(
                "Tool {tool_name} failed: {error_msg}"
            )));
        }

        result
            .structured_content
            .ok_or_else(|| {
                AgentServiceError::Internal(format!(
                    "Tool {tool_name} returned no structured_content"
                ))
            })
    }
}
