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

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Tool};
use systemprompt_extension::brain::{BRAIN_TOOLSET, DynBrainToolsetSource};
use systemprompt_extension::mcp::{McpToolset, ToolsetCall};
use systemprompt_identifiers::McpServerId;

use systemprompt_documents::links::LinkIssueError;
use systemprompt_documents::mcp::{DocumentsMcpTools, DocumentsToolCall};
use systemprompt_files::documents::DocumentsUnbound;
use systemprompt_issues::mcp::{IssuesMcpTools, IssuesToolCall};

use super::factory::ToolsetBuildResult;
use crate::HostContext;
use crate::contract::ToolsetSlotBuilder;

pub use systemprompt_documents::mcp::TOOLSET_ID as DOCUMENTS_TOOLSET;
pub use systemprompt_issues::mcp::TOOLSET_ID as ISSUES_TOOLSET;

/// Why the issue report tools could not be built for a hosted server.
#[derive(Debug, thiserror::Error)]
pub enum IssuesMcpError {
    #[error(
        "MCP server {server_name} serves issue report tools but issue reports are disabled: set \
         files.issues.enabled: true in services/config/files.yaml"
    )]
    Disabled { server_name: McpServerId },
    #[error("MCP server {server_name}: the API link client could not be built")]
    Links {
        server_name: McpServerId,
        #[source]
        source: LinkIssueError,
    },
    #[error("MCP server {server_name}: the document store is not bound on this host")]
    Unbound {
        server_name: McpServerId,
        #[source]
        source: DocumentsUnbound,
    },
}

/// Why the document tools could not be built for a hosted server.
#[derive(Debug, thiserror::Error)]
pub enum DocumentsMcpError {
    #[error(
        "MCP server {server_name} serves document tools but the document store is disabled: set \
         files.documents.enabled: true in services/config/files.yaml"
    )]
    Disabled { server_name: McpServerId },
    #[error("MCP server {server_name}: the API link client could not be built")]
    Links {
        server_name: McpServerId,
        #[source]
        source: LinkIssueError,
    },
    #[error("MCP server {server_name}: the document store is not bound on this host")]
    Unbound {
        server_name: McpServerId,
        #[source]
        source: DocumentsUnbound,
    },
}

pub fn issues_tools(
    host: &HostContext,
    server_name: McpServerId,
) -> Result<IssuesMcpTools, IssuesMcpError> {
    if !host.files_config().issues_config().is_enabled() {
        return Err(IssuesMcpError::Disabled { server_name });
    }
    let handles = host.documents().map_err(|source| IssuesMcpError::Unbound {
        server_name: server_name.clone(),
        source,
    })?;
    let uploads = host
        .files_config()
        .documents_config()
        .is_enabled()
        .then(|| Arc::clone(&handles.uploads));
    let links = host
        .link_issuer(&server_name)
        .map_err(|source| IssuesMcpError::Links {
            server_name: server_name.clone(),
            source,
        })?;
    Ok(IssuesMcpTools::new(
        Arc::clone(&handles.issues),
        uploads,
        links,
        server_name,
    ))
}

pub fn documents_tools(
    host: &HostContext,
    server_name: McpServerId,
) -> Result<DocumentsMcpTools, DocumentsMcpError> {
    if !host.files_config().documents_config().is_enabled() {
        return Err(DocumentsMcpError::Disabled { server_name });
    }
    let links = host
        .link_issuer(&server_name)
        .map_err(|source| DocumentsMcpError::Links {
            server_name: server_name.clone(),
            source,
        })?;
    let handles = host
        .documents()
        .map_err(|source| DocumentsMcpError::Unbound {
            server_name: server_name.clone(),
            source,
        })?;
    DocumentsMcpTools::for_host(
        handles,
        Arc::clone(host.user_repository()),
        host.state(),
        links,
        server_name.clone(),
    )
    .ok_or(DocumentsMcpError::Disabled { server_name })
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
        systemprompt_documents::mcp::INSTRUCTIONS
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
