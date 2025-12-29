use anyhow::Result;
use std::sync::Arc;
use systemprompt_identifiers::AgentName;
use systemprompt_models::RequestContext;
use systemprompt_traits::{ToolDefinition, ToolProvider};

use crate::models::tools::McpTool;

use super::adapter::{definition_to_mcp_tool, request_context_to_tool_context};

pub struct ToolDiscovery {
    tool_provider: Arc<dyn ToolProvider>,
}

impl std::fmt::Debug for ToolDiscovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolDiscovery").finish_non_exhaustive()
    }
}

impl ToolDiscovery {
    pub fn new(tool_provider: Arc<dyn ToolProvider>) -> Self {
        Self { tool_provider }
    }

    pub async fn discover_tools(
        &self,
        agent_name: &AgentName,
        context: &RequestContext,
    ) -> Result<Vec<McpTool>> {
        self.tool_provider
            .refresh_connections(agent_name.as_str())
            .await?;

        let tool_context = request_context_to_tool_context(context);
        let definitions = self
            .tool_provider
            .list_tools(agent_name.as_str(), &tool_context)
            .await?;

        Ok(definitions.iter().map(definition_to_mcp_tool).collect())
    }

    pub async fn find_tool_for_agent(
        &self,
        agent_name: &AgentName,
        tool_name: &str,
        context: &RequestContext,
    ) -> Result<Option<McpTool>> {
        let tool_context = request_context_to_tool_context(context);
        let definition = self
            .tool_provider
            .find_tool(agent_name.as_str(), tool_name, &tool_context)
            .await?;

        Ok(definition.as_ref().map(definition_to_mcp_tool))
    }

    pub fn definitions_to_mcp_tools(definitions: &[ToolDefinition]) -> Vec<McpTool> {
        definitions.iter().map(definition_to_mcp_tool).collect()
    }
}
