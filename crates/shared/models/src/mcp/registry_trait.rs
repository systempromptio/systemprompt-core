//! MCP registry trait.
//!
//! [`McpRegistry`] is called on concrete implementations, never held as a
//! trait object, so it declares native `async` methods.
//! Every fallible method returns
//! [`McpRegistryResult`](crate::errors::McpRegistryResult).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

use crate::errors::McpRegistryResult as Result;

use systemprompt_identifiers::McpServerId;

#[derive(Debug, Clone)]
pub struct McpServerState {
    pub name: McpServerId,
    pub host: String,
    pub port: Option<u16>,
}

pub trait McpRegistry: Send + Sync {
    fn list_servers(&self) -> impl Future<Output = Result<Vec<McpServerId>>> + Send;

    fn find_server(
        &self,
        name: &McpServerId,
    ) -> impl Future<Output = Result<Option<McpServerState>>> + Send;

    fn server_exists(&self, name: &McpServerId) -> impl Future<Output = Result<bool>> + Send;
}
