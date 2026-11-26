//! The `issues` toolset the issues member contributes to the MCP host's
//! `ToolsetSlot`.
//!
//! A composition without the issues member registers no `issues` toolset:
//! an enabled hosted server that lists it is refused when the slot is built,
//! and no host lists or calls its tools.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Tool};
use systemprompt_documents::links::hosted_link_issuer;
use systemprompt_extension::mcp::{McpToolset, ToolsetCall};
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp_host::HostContext;
use systemprompt_mcp_host::contract::ToolsetSlotBuilder;
use systemprompt_mcp_host::toolsets::ToolsetBuildResult;

use super::{INSTRUCTIONS, IssuesMcpError, IssuesMcpTools, IssuesToolCall, TOOLSET_ID};

pub fn issues_tools(
    host: &HostContext,
    server_name: McpServerId,
) -> Result<IssuesMcpTools, IssuesMcpError> {
    let links_for = server_name.clone();
    IssuesMcpTools::hosted(
        host.state(),
        || hosted_link_issuer(host, &links_for),
        server_name,
    )
}

#[derive(Debug)]
struct IssuesToolset(IssuesMcpTools);

#[async_trait]
impl McpToolset for IssuesToolset {
    fn id(&self) -> &str {
        TOOLSET_ID
    }

    fn instructions(&self) -> &'static str {
        INSTRUCTIONS
    }

    fn tools(&self) -> Vec<Tool> {
        self.0.list_tools()
    }

    async fn call(&self, call: ToolsetCall<'_>) -> Result<CallToolResult, McpError> {
        self.0
            .call(IssuesToolCall {
                executor: call.executor,
                request: call.request,
                caller: call.caller,
                client: call.client,
                hop: call.hop,
            })
            .await
    }
}

fn build(host: &HostContext, server: &McpServerId) -> ToolsetBuildResult {
    let tools = issues_tools(host, server.clone())?;
    Ok(Some(Box::new(IssuesToolset(tools))))
}

pub(crate) fn contribute(slot: &mut ToolsetSlotBuilder) {
    slot.toolset(TOOLSET_ID, build);
}
