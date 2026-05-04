//! [`ToolProvider`] trait — discovery + invocation contract.

use async_trait::async_trait;
use std::collections::HashMap;

use super::call::{ToolCallRequest, ToolCallResult};
use super::context::ToolContext;
use super::definition::ToolDefinition;
use super::error::ToolProviderResult;

/// Discovery + invocation contract for a tool backend.
///
/// Marked `#[async_trait]` because it is consumed via `dyn ToolProvider`.
#[async_trait]
pub trait ToolProvider: Send + Sync {
    /// List the tools available to `agent_name` under `context`.
    async fn list_tools(
        &self,
        agent_name: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<Vec<ToolDefinition>>;

    /// Invoke a tool on the backing service identified by `service_id`.
    async fn call_tool(
        &self,
        request: &ToolCallRequest,
        service_id: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<ToolCallResult>;

    /// Refresh per-agent backend connections (e.g. MCP server handshakes).
    async fn refresh_connections(&self, agent_name: &str) -> ToolProviderResult<()>;

    /// Liveness check; returns a `service_id -> healthy` map.
    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>>;

    /// Default lookup of one tool by name; provided for convenience.
    async fn find_tool(
        &self,
        agent_name: &str,
        tool_name: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<Option<ToolDefinition>> {
        let tools = self.list_tools(agent_name, context).await?;
        Ok(tools.into_iter().find(|t| t.name == tool_name))
    }
}
