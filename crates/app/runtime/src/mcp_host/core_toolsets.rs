//! The toolsets core itself registers: `issues` and `documents`.
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
use crate::AppContext;
use crate::documents::mcp::{DocumentsMcpTools, DocumentsToolCall};
use crate::issues::mcp::{IssuesMcpTools, IssuesToolCall};

pub const ISSUES_TOOLSET: &str = "issues";
pub const DOCUMENTS_TOOLSET: &str = "documents";

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
        }))
        .await
    }
}

fn build_issues(ctx: &AppContext, server: &McpServerId) -> ToolsetBuildResult {
    let tools = IssuesMcpTools::from_context(ctx, server.clone())?;
    Ok(Some(Box::new(IssuesToolset(tools))))
}

fn build_documents(ctx: &AppContext, server: &McpServerId) -> ToolsetBuildResult {
    let tools = DocumentsMcpTools::from_context(ctx, server.clone())?;
    Ok(Some(Box::new(DocumentsToolset(tools))))
}

crate::submit_toolset!(ISSUES_TOOLSET, build_issues);
crate::submit_toolset!(DOCUMENTS_TOOLSET, build_documents);
