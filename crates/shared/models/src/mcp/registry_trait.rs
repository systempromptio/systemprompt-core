//! MCP registry and provider traits.
//!
//! [`McpRegistry`], [`McpToolProvider`] and [`McpDeploymentProvider`] are
//! held as the `Dyn*` aliases (`Arc<dyn _>`) by the OAuth and agent domains,
//! so they use `#[async_trait]`; native `async fn` in traits is not
//! `dyn`-compatible. Every method returns
//! [`McpRegistryResult`](crate::errors::McpRegistryResult).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::errors::McpRegistryResult as Result;

use crate::ai::tools::McpTool;
use crate::execution::context::RequestContext;
use systemprompt_identifiers::McpServerId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum McpServerStatus {
    Unknown,
    Running,
    Stopped,
}

#[derive(Debug, Clone)]
pub struct McpServerState {
    pub name: McpServerId,
    pub host: String,
    pub port: Option<u16>,
    pub status: McpServerStatus,
}

#[async_trait]
pub trait McpRegistry: Send + Sync {
    async fn list_servers(&self) -> Result<Vec<McpServerId>>;

    async fn find_server(&self, name: &McpServerId) -> Result<Option<McpServerState>>;

    async fn server_exists(&self, name: &McpServerId) -> Result<bool>;
}

#[async_trait]
pub trait McpToolProvider: Send + Sync {
    async fn list_tools(
        &self,
        server_name: &McpServerId,
        context: &RequestContext,
    ) -> Result<Vec<McpTool>>;

    async fn load_tools_for_servers(
        &self,
        server_names: &[McpServerId],
        context: &RequestContext,
    ) -> Result<HashMap<McpServerId, Vec<McpTool>>>;
}

#[async_trait]
pub trait McpDeploymentProvider: Send + Sync {
    async fn load_config(&self) -> Result<crate::ServicesConfig>;

    fn protocol_version(&self) -> &str;
}

pub type DynMcpRegistry = Arc<dyn McpRegistry>;

pub type DynMcpToolProvider = Arc<dyn McpToolProvider>;

pub type DynMcpDeploymentProvider = Arc<dyn McpDeploymentProvider>;
