//! The toolsets core itself registers: `issues`, `documents` and, with the
//! `brain` feature, `brain`.
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
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::{McpToolset, ToolsetCall};

use super::factory::ToolsetBuildResult;
use crate::HostContext;
use crate::documents::mcp::{DocumentsMcpTools, DocumentsToolCall};
use crate::issues::mcp::{IssuesMcpTools, IssuesToolCall};

pub const ISSUES_TOOLSET: &str = "issues";
pub const DOCUMENTS_TOOLSET: &str = "documents";
#[cfg(feature = "brain")]
const BRAIN_TOOLSET: &str = "brain";

#[derive(Debug)]
struct IssuesToolset(IssuesMcpTools);

#[async_trait]
impl McpToolset for IssuesToolset {
    fn id(&self) -> &str {
        ISSUES_TOOLSET
    }

    fn instructions(&self) -> &'static str {
        crate::issues::mcp::INSTRUCTIONS
    }

    fn tools(&self) -> Vec<Tool> {
        self.0.list_tools()
    }

    async fn call(&self, call: ToolsetCall<'_>) -> Result<CallToolResult, McpError> {
        self.0
            .call(IssuesToolCall {
                executor: call.executor,
                request: call.request,
                context: call.context,
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
        crate::documents::mcp::INSTRUCTIONS
    }

    fn tools(&self) -> Vec<Tool> {
        self.0.list_tools()
    }

    async fn call(&self, call: ToolsetCall<'_>) -> Result<CallToolResult, McpError> {
        Box::pin(self.0.call(DocumentsToolCall {
            executor: call.executor,
            request: call.request,
            context: call.context,
            client: call.client,
            hop: call.hop,
        }))
        .await
    }
}

fn build_issues(host: &HostContext, server: &McpServerId) -> ToolsetBuildResult {
    let tools = IssuesMcpTools::from_host(host, server.clone())?;
    Ok(Some(Box::new(IssuesToolset(tools))))
}

fn build_documents(host: &HostContext, server: &McpServerId) -> ToolsetBuildResult {
    let tools = DocumentsMcpTools::from_host(host, server.clone())?;
    Ok(Some(Box::new(DocumentsToolset(tools))))
}

crate::submit_toolset!(ISSUES_TOOLSET, build_issues);
crate::submit_toolset!(DOCUMENTS_TOOLSET, build_documents);

// Why: the agent half of the brain only. Confirming, retracting, pinning,
// erasing and resolving stay a person's acts over HTTP and the console, so
// the toolset carries `TOOL_NAMES` and never `GOVERNING_TOOLS`.
#[cfg(feature = "brain")]
#[derive(Debug)]
struct BrainToolset(crate::brain::mcp::BrainMcpTools, McpServerId);

#[cfg(feature = "brain")]
#[async_trait]
impl McpToolset for BrainToolset {
    fn id(&self) -> &str {
        BRAIN_TOOLSET
    }

    fn instructions(&self) -> &'static str {
        crate::brain::mcp::INSTRUCTIONS
    }

    fn tools(&self) -> Vec<Tool> {
        self.0.list_tools(&self.1)
    }

    async fn call(&self, call: ToolsetCall<'_>) -> Result<CallToolResult, McpError> {
        Box::pin(self.0.call(crate::brain::mcp::BrainToolCall {
            executor: call.executor,
            request: call.request,
            context: call.context,
            client: call.client,
        }))
        .await
    }
}

// Why: `Ok(None)` while the profile has the brain off, so a server that lists
// the toolset still boots and simply offers no brain tools.
#[cfg(feature = "brain")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is the ToolsetFactory build pointer"
)]
fn build_brain(host: &HostContext, server: &McpServerId) -> ToolsetBuildResult {
    let Some(Ok(tools)) = host.brain().map(crate::brain::BrainHandle::mcp_tools) else {
        return Ok(None);
    };
    Ok(Some(Box::new(BrainToolset(tools, server.clone()))))
}

#[cfg(feature = "brain")]
crate::submit_toolset!(BRAIN_TOOLSET, build_brain);
