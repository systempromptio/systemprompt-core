//! The brain's agent toolset, built from the bound brain handle.
//!
//! The toolset carries [`TOOL_NAMES`](super::TOOL_NAMES) only. Confirming,
//! retracting, pinning, erasing and resolving stay a person's acts over HTTP
//! and the console, so a governing tool name is refused as an agent act.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Tool};
use systemprompt_extension::brain::{BRAIN_TOOLSET, BrainToolsetSource};
use systemprompt_extension::mcp::{McpToolset, ToolsetCall};
use systemprompt_identifiers::McpServerId;

use super::{BrainMcpTools, BrainToolCall, INSTRUCTIONS};
use crate::handle::BrainHandleSlot;

#[derive(Debug)]
struct BrainToolset {
    tools: BrainMcpTools,
    server: McpServerId,
}

#[async_trait]
impl McpToolset for BrainToolset {
    fn id(&self) -> &str {
        BRAIN_TOOLSET
    }

    fn instructions(&self) -> &'static str {
        INSTRUCTIONS
    }

    fn tools(&self) -> Vec<Tool> {
        self.tools.list_tools(&self.server)
    }

    async fn call(&self, call: ToolsetCall<'_>) -> Result<CallToolResult, McpError> {
        Box::pin(self.tools.call(BrainToolCall {
            executor: call.executor,
            request: call.request,
            context: call.context,
            client: call.client,
        }))
        .await
    }
}

pub(crate) struct BoundBrainToolsets(Arc<BrainHandleSlot>);

impl BoundBrainToolsets {
    pub(crate) const fn new(brain: Arc<BrainHandleSlot>) -> Self {
        Self(brain)
    }
}

impl BrainToolsetSource for BoundBrainToolsets {
    fn toolset(&self, server: &McpServerId) -> Option<Box<dyn McpToolset>> {
        let handle = self.0.get()?;
        let tools = BrainMcpTools::from_parts(
            Arc::clone(handle.service()),
            handle.memory_recall(),
            Arc::clone(handle.profile()),
        )
        .ok()?;
        Some(Box::new(BrainToolset {
            tools,
            server: server.clone(),
        }))
    }
}
