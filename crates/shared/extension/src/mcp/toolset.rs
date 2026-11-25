//! Declarative MCP toolsets: a server's tools as compiled units its YAML hosts.
//!
//! An [`McpToolset`] is a named group of tools a binary links in. A deployment
//! lists the ids it hosts under `toolsets:` and core's generic host
//! (`systemprompt_mcp_host::serving::host::ToolsetServer`) serves their union:
//! it filters the tool list per caller and gates every call on the access the
//! YAML resolves for that tool. The toolset itself only lists its tools and
//! executes a call that has already been authenticated and authorised.
//!
//! Factories that build a toolset from the host context live in
//! `systemprompt-mcp-host`, which sits above this crate.
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
use systemprompt_identifiers::JwtToken;
use systemprompt_models::mcp::ClientProfile;

use super::caller::VerifiedCaller;
use super::execution::ToolExecution;

/// One resource read, already authenticated by the host, with the caller the
/// platform proxy verified: the same caller a [`ToolsetCall`] carries.
#[derive(Debug, Clone, Copy)]
pub struct ResourceRead<'a> {
    pub request: &'a ReadResourceRequestParams,
    pub caller: &'a VerifiedCaller,
}

/// One tool call, already authenticated and authorised by the host.
#[derive(Debug, Clone, Copy)]
pub struct ToolsetCall<'a> {
    pub executor: &'a dyn ToolExecution,
    pub request: &'a CallToolRequestParams,
    pub caller: &'a VerifiedCaller,
    pub client: &'a ClientProfile,
    pub hop: Option<&'a JwtToken>,
}

/// A named group of tools one MCP server can host.
///
/// `instructions` is joined into the server's instructions (empty adds none).
/// A toolset that serves resources answers `true` from `serves_resources`, so
/// the host advertises the capability; its `read_resource` answers `None` for
/// a URI it does not own, so the host asks the next toolset. The host reads a
/// resource only for a caller the platform proxy verified, and hands that
/// caller to `read_resource`, so a toolset scopes what it serves to it.
///
/// `#[async_trait]` because the host holds toolsets as `Box<dyn McpToolset>`,
/// which native `async fn` in traits cannot be.
#[async_trait]
pub trait McpToolset: Send + Sync + Debug {
    fn id(&self) -> &str;

    fn instructions(&self) -> &'static str {
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

    async fn read_resource(
        &self,
        _read: ResourceRead<'_>,
    ) -> Option<Result<ReadResourceResult, McpError>> {
        None
    }
}
