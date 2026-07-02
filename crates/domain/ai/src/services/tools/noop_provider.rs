//! A [`systemprompt_traits::ToolProvider`] that exposes no tools.
//!
//! Used as the default provider when a request has no tool backend wired up, so
//! call sites can rely on an always-present provider rather than an `Option`.

use async_trait::async_trait;
use std::collections::HashMap;
use systemprompt_identifiers::McpServerId;
use systemprompt_traits::{
    ToolCallRequest, ToolCallResult, ToolContext, ToolDefinition, ToolProvider, ToolProviderError,
    ToolProviderResult,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopToolProvider;

impl NoopToolProvider {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ToolProvider for NoopToolProvider {
    async fn list_tools(
        &self,
        _agent_name: &str,
        _context: &ToolContext,
    ) -> ToolProviderResult<Vec<ToolDefinition>> {
        Ok(Vec::new())
    }

    async fn call_tool(
        &self,
        request: &ToolCallRequest,
        _service_id: &McpServerId,
        _context: &ToolContext,
    ) -> ToolProviderResult<ToolCallResult> {
        Err(ToolProviderError::ServiceNotFound(format!(
            "NoopToolProvider cannot execute tool: {}",
            request.name
        )))
    }

    async fn refresh_connections(&self, _agent_name: &str) -> ToolProviderResult<()> {
        Ok(())
    }

    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>> {
        Ok(HashMap::new())
    }
}
