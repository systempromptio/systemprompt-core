//! [`ToolProvider`] trait — discovery + invocation contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::collections::HashMap;

use systemprompt_identifiers::McpServerId;

use super::call::{ToolCallRequest, ToolCallResult};
use super::context::ToolContext;
use super::definition::ToolDefinition;
use super::error::ToolProviderResult;

// Why: provider is consumed as a trait object so tool backends swap at profile
// level; an async fn in a bare trait is not dyn-compatible, so #[async_trait]
// is required.
#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn list_tools(
        &self,
        agent_name: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<Vec<ToolDefinition>>;

    async fn call_tool(
        &self,
        request: &ToolCallRequest,
        service_id: &McpServerId,
        context: &ToolContext,
    ) -> ToolProviderResult<ToolCallResult>;

    async fn refresh_connections(&self, agent_name: &str) -> ToolProviderResult<()>;

    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>>;

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
