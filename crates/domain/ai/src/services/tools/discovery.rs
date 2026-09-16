//! Discovery of the tool definitions available to a request.
//!
//! Queries the registered [`systemprompt_traits::ToolProvider`]s for the tools
//! an agent may call in a given [`systemprompt_models::RequestContext`],
//! producing the definitions passed to the model.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{AiError, Result};
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
        self.tool_provider.refresh_connections(agent_name).await?;

        let tool_context = request_context_to_tool_context(context);
        let inventory = self
            .tool_provider
            .list_tools(agent_name, &tool_context)
            .await?;

        // Why: an agent planning against a partial tool list would silently
        // lose capabilities; a server that cannot be listed fails discovery.
        if !inventory.is_complete() {
            let failed: Vec<String> = inventory
                .failed_servers
                .iter()
                .map(|f| format!("{}: {}", f.server, f.message))
                .collect();
            return Err(AiError::ToolDiscovery(format!(
                "tool inventory for agent {agent_name} is incomplete: {}",
                failed.join("; ")
            )));
        }

        Ok(inventory.tools.iter().map(definition_to_mcp_tool).collect())
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
            .find_tool(agent_name, tool_name, &tool_context)
            .await?;

        Ok(definition.as_ref().map(definition_to_mcp_tool))
    }

    pub fn definitions_to_mcp_tools(definitions: &[ToolDefinition]) -> Vec<McpTool> {
        definitions.iter().map(definition_to_mcp_tool).collect()
    }
}
