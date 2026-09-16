//! [`ToolProvider`] trait — discovery + invocation contract.
//!
//! [`ToolProvider`] is held as `Arc<dyn ToolProvider>` by the AI domain's
//! tool discovery and executor, so it uses `#[async_trait]`; native
//! `async fn` in traits is not `dyn`-compatible.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::collections::HashMap;

use systemprompt_identifiers::{AgentName, McpServerId};

use super::call::{ToolCallRequest, ToolCallResult};
use super::context::ToolContext;
use super::definition::ToolDefinition;
use super::error::ToolProviderResult;
use super::inventory::ToolInventory;

#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn list_tools(
        &self,
        agent_name: &AgentName,
        context: &ToolContext,
    ) -> ToolProviderResult<ToolInventory>;

    async fn call_tool(
        &self,
        request: &ToolCallRequest,
        service_id: &McpServerId,
        context: &ToolContext,
    ) -> ToolProviderResult<ToolCallResult>;

    async fn refresh_connections(&self, agent_name: &AgentName) -> ToolProviderResult<()>;

    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>>;

    async fn find_tool(
        &self,
        agent_name: &AgentName,
        tool_name: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<Option<ToolDefinition>> {
        let inventory = self.list_tools(agent_name, context).await?;
        Ok(inventory.tools.into_iter().find(|t| t.name == tool_name))
    }
}
