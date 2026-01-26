use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::AgentName;
use systemprompt_models::{AiProvider, CallToolResult, McpTool, RequestContext, ToolCall};

use crate::models::a2a::Artifact;
use crate::services::mcp::McpToA2aTransformer;

#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn list_available_tools_for_agent(
        &self,
        agent_name: &AgentName,
        context: &RequestContext,
    ) -> Result<Vec<McpTool>>;
}

pub struct AiServiceToolProvider {
    ai_service: Arc<dyn AiProvider>,
}

impl std::fmt::Debug for AiServiceToolProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiServiceToolProvider")
            .field("ai_service", &"<AiProvider>")
            .finish()
    }
}

impl AiServiceToolProvider {
    pub fn new(ai_service: Arc<dyn AiProvider>) -> Self {
        Self { ai_service }
    }
}

#[async_trait]
impl ToolProvider for AiServiceToolProvider {
    async fn list_available_tools_for_agent(
        &self,
        agent_name: &AgentName,
        context: &RequestContext,
    ) -> Result<Vec<McpTool>> {
        self.ai_service
            .list_available_tools_for_agent(agent_name, context)
            .await
    }
}

#[derive(Debug)]
pub struct ArtifactBuilder {
    tool_calls: Vec<ToolCall>,
    tool_results: Vec<CallToolResult>,
    tools: Vec<McpTool>,
    context_id: String,
    task_id: String,
}

impl ArtifactBuilder {
    pub fn new(
        tool_calls: Vec<ToolCall>,
        tool_results: Vec<CallToolResult>,
        tools: Vec<McpTool>,
        context_id: String,
        task_id: String,
    ) -> Self {
        Self {
            tool_calls,
            tool_results,
            tools,
            context_id,
            task_id,
        }
    }

    fn get_output_schema(&self, tool_name: &str) -> Option<&serde_json::Value> {
        self.tools
            .iter()
            .find(|t| t.name == tool_name)
            .and_then(|t| t.output_schema.as_ref())
    }

    pub fn build_artifacts(&self) -> Result<Vec<Artifact>> {
        let mut artifacts = Vec::new();

        for (index, result) in self.tool_results.iter().enumerate() {
            if let Some(structured_content) =
                result.structured_content.as_ref().filter(|v| !v.is_null())
            {
                if let Some(tool_call) = self.tool_calls.get(index) {
                    let output_schema = self.get_output_schema(&tool_call.name);

                    let mut artifact = McpToA2aTransformer::transform_from_json(
                        &tool_call.name,
                        structured_content,
                        output_schema,
                        &self.context_id,
                        &self.task_id,
                        Some(&tool_call.arguments),
                    )
                    .map_err(|e| {
                        anyhow::anyhow!("Tool '{}' artifact transform failed: {e}", tool_call.name)
                    })?;

                    artifact.metadata = artifact.metadata.with_execution_index(index);

                    artifacts.push(artifact);
                }
            }
        }

        Ok(artifacts)
    }
}
