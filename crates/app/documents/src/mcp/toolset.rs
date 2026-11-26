//! The `documents` toolset the documents member contributes to the MCP
//! host's `ToolsetSlot`.
//!
//! A composition without the documents member registers no `documents`
//! toolset: an enabled hosted server that lists it is refused when the slot
//! is built, and no host lists or calls its tools.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Tool};
use systemprompt_extension::mcp::{McpToolset, ToolsetCall};
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp_host::HostContext;
use systemprompt_mcp_host::contract::ToolsetSlotBuilder;
use systemprompt_mcp_host::toolsets::ToolsetBuildResult;

use super::{DocumentsMcpError, DocumentsMcpTools, DocumentsToolCall, TOOLSET_ID};
use crate::links::hosted_link_issuer;

pub fn documents_tools(
    host: &HostContext,
    server_name: McpServerId,
) -> Result<DocumentsMcpTools, DocumentsMcpError> {
    let links_for = server_name.clone();
    DocumentsMcpTools::hosted(
        host.state(),
        || hosted_link_issuer(host, &links_for),
        server_name,
    )
}

#[derive(Debug)]
struct DocumentsToolset(DocumentsMcpTools);

#[async_trait]
impl McpToolset for DocumentsToolset {
    fn id(&self) -> &str {
        TOOLSET_ID
    }

    fn instructions(&self) -> &'static str {
        self.0.instructions()
    }

    fn tools(&self) -> Vec<Tool> {
        self.0.list_tools()
    }

    async fn call(&self, call: ToolsetCall<'_>) -> Result<CallToolResult, McpError> {
        Box::pin(self.0.call(DocumentsToolCall {
            executor: call.executor,
            request: call.request,
            caller: call.caller,
            client: call.client,
            hop: call.hop,
        }))
        .await
    }
}

fn build(host: &HostContext, server: &McpServerId) -> ToolsetBuildResult {
    let tools = documents_tools(host, server.clone())?;
    Ok(Some(Box::new(DocumentsToolset(tools))))
}

pub(crate) fn contribute(slot: &mut ToolsetSlotBuilder) {
    slot.toolset(TOOLSET_ID, build);
}
