use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::{PendingToolCall, RiskLevel, ToolRegistry, ToolResult};
use crate::messages::{Message, ToolExecutionResult};

pub struct ToolExecutor {
    registry: Arc<ToolRegistry>,
    context: Arc<()>,
    message_tx: mpsc::UnboundedSender<Message>,
}

impl ToolExecutor {
    pub fn new(registry: Arc<ToolRegistry>, message_tx: mpsc::UnboundedSender<Message>) -> Self {
        Self {
            registry,
            context: Arc::new(()),
            message_tx,
        }
    }

    pub fn create_pending_tool_call(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Option<PendingToolCall> {
        let tool = self.registry.get(tool_name)?;

        Some(PendingToolCall {
            id: Uuid::new_v4(),
            tool_name: tool_name.to_string(),
            arguments: arguments.clone(),
            description: tool.description().to_string(),
            risk_level: tool.risk_level(),
            preview: tool.preview(arguments),
        })
    }

    pub fn should_auto_approve(&self, tool_name: &str) -> bool {
        self.registry
            .get(tool_name)
            .is_some_and(|t| !t.requires_approval() || t.risk_level() == RiskLevel::Safe)
    }

    pub async fn execute(&self, tool_call: PendingToolCall) -> Result<ToolResult> {
        let tool = self
            .registry
            .get(&tool_call.tool_name)
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", tool_call.tool_name))?;

        tool.execute(tool_call.arguments, &self.context).await
    }

    pub fn spawn_execution(&self, tool_call: PendingToolCall) {
        let registry = Arc::clone(&self.registry);
        let context = Arc::clone(&self.context);
        let tx = self.message_tx.clone();
        let tool_id = tool_call.id;

        tokio::spawn(async move {
            let result = if let Some(tool) = registry.get(&tool_call.tool_name) {
                match tool.execute(tool_call.arguments, &context).await {
                    Ok(result) => ToolExecutionResult {
                        success: result.success,
                        output: result.output,
                        error: result.error,
                    },
                    Err(e) => ToolExecutionResult {
                        success: false,
                        output: String::new(),
                        error: Some(e.to_string()),
                    },
                }
            } else {
                ToolExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Tool not found: {}", tool_call.tool_name)),
                }
            };

            let _ = tx.send(Message::ToolExecutionComplete(tool_id, result));
        });
    }
}

impl std::fmt::Debug for ToolExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolExecutor")
            .field("registry", &self.registry)
            .finish_non_exhaustive()
    }
}
