//! The toolsets core itself contributes: `issues`, `documents` and `brain`.
//!
//! The `brain` toolset is built from the `DynBrainToolsetSource` the brain
//! member provides; it offers no tools when that member is not admitted or
//! the profile has the brain off, so a server that lists it still boots.
//!
//! Each wraps the public tool library of the same name, so a server that
//! composes the library by hand keeps working while a YAML-declared server
//! hosts it with `toolsets: [{id: issues}]`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Tool};
use systemprompt_extension::brain::{BRAIN_TOOLSET, DynBrainToolsetSource};
use systemprompt_extension::mcp::{McpToolset, ToolsetCall};
use systemprompt_identifiers::McpServerId;

use systemprompt_documents::mcp::{DocumentsMcpError, DocumentsMcpTools, DocumentsToolCall};
use systemprompt_issues::mcp::{IssuesMcpError, IssuesMcpTools, IssuesToolCall};

use super::factory::ToolsetBuildResult;
use crate::HostContext;
use crate::contract::ToolsetSlotBuilder;

pub use systemprompt_documents::mcp::TOOLSET_ID as DOCUMENTS_TOOLSET;
pub use systemprompt_issues::mcp::TOOLSET_ID as ISSUES_TOOLSET;

pub fn issues_tools(
    host: &HostContext,
    server_name: McpServerId,
) -> Result<IssuesMcpTools, IssuesMcpError> {
    let links_for = server_name.clone();
    IssuesMcpTools::hosted(host.state(), || host.link_issuer(&links_for), server_name)
}

pub fn documents_tools(
    host: &HostContext,
    server_name: McpServerId,
) -> Result<DocumentsMcpTools, DocumentsMcpError> {
    let links_for = server_name.clone();
    DocumentsMcpTools::hosted(host.state(), || host.link_issuer(&links_for), server_name)
}

#[derive(Debug)]
struct IssuesToolset(IssuesMcpTools);

#[async_trait]
impl McpToolset for IssuesToolset {
    fn id(&self) -> &str {
        ISSUES_TOOLSET
    }

    fn instructions(&self) -> &'static str {
        systemprompt_issues::mcp::INSTRUCTIONS
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

#[derive(Debug)]
struct DocumentsToolset(DocumentsMcpTools);

#[async_trait]
impl McpToolset for DocumentsToolset {
    fn id(&self) -> &str {
        DOCUMENTS_TOOLSET
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

fn build_issues(host: &HostContext, server: &McpServerId) -> ToolsetBuildResult {
    let tools = issues_tools(host, server.clone())?;
    Ok(Some(Box::new(IssuesToolset(tools))))
}

fn build_documents(host: &HostContext, server: &McpServerId) -> ToolsetBuildResult {
    let tools = documents_tools(host, server.clone())?;
    Ok(Some(Box::new(DocumentsToolset(tools))))
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is the ToolsetBuildFn pointer"
)]
fn build_brain(host: &HostContext, server: &McpServerId) -> ToolsetBuildResult {
    Ok(host
        .state()
        .find::<DynBrainToolsetSource>()
        .and_then(|source| source.toolset(server)))
}

pub(crate) fn contribute(slot: &mut ToolsetSlotBuilder) {
    slot.toolset(ISSUES_TOOLSET, build_issues)
        .toolset(DOCUMENTS_TOOLSET, build_documents)
        .toolset(BRAIN_TOOLSET, build_brain);
}
