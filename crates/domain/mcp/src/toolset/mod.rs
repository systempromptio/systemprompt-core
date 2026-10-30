//! Declarative MCP toolsets: a server's tools as compiled units its YAML hosts.
//!
//! An [`McpToolset`] is a named group of tools a binary links in. A deployment
//! lists the ids it hosts under `toolsets:` and core's generic host
//! ([`crate::host::ToolsetServer`]) serves their union: it filters the tool
//! list per caller and gates every call on the access the YAML resolves for
//! that tool. The toolset itself only lists its tools and executes a call that
//! has already been authenticated and authorised.
//!
//! Factories that build a toolset from the application context live with that
//! context, in the runtime crate (`submit_toolset!`), because this crate sits
//! below it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt::Debug;

use async_trait::async_trait;
use rmcp::ErrorData as McpError;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListResourcesResult, ReadResourceRequestParams,
    ReadResourceResult, Tool,
};
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::ClientProfile;

use crate::tool::McpToolExecutor;

/// One tool call, already authenticated and authorised by the host.
#[derive(Debug, Clone, Copy)]
pub struct ToolsetCall<'a> {
    pub executor: &'a McpToolExecutor,
    pub request: &'a CallToolRequestParams,
    pub context: &'a RequestContext,
    pub client: &'a ClientProfile,
}

/// A named group of tools one MCP server can host.
///
/// `instructions` is joined into the server's instructions (empty adds none).
/// A toolset that serves resources answers `true` from `serves_resources`, so
/// the host advertises the capability; its `read_resource` answers `None` for
/// a URI it does not own, so the host asks the next toolset.
///
/// `#[async_trait]` because the host holds toolsets as `Box<dyn McpToolset>`,
/// which native `async fn` in traits cannot be.
#[async_trait]
pub trait McpToolset: Send + Sync + Debug {
    fn id(&self) -> &str;

    fn instructions(&self) -> &str {
        ""
    }

    fn tools(&self) -> Vec<Tool>;

    async fn call(&self, call: ToolsetCall<'_>) -> Result<CallToolResult, McpError>;

    fn serves_resources(&self) -> bool {
        false
    }

    fn list_resources(&self) -> ListResourcesResult {
        ListResourcesResult::default()
    }

    fn read_resource(
        &self,
        _request: &ReadResourceRequestParams,
    ) -> Option<Result<ReadResourceResult, McpError>> {
        None
    }
}
