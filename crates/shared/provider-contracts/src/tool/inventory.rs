//! The typed partial outcome of a tool listing across several servers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::McpServerId;

use super::definition::ToolDefinition;

/// Tools gathered from every reachable server plus the servers that could
/// not be listed.
///
/// A consumer that needs the full inventory must treat a non-empty
/// `failed_servers` as a failure, never as "fewer tools".
#[derive(Debug, Clone, Default)]
pub struct ToolInventory {
    pub tools: Vec<ToolDefinition>,
    pub failed_servers: Vec<ServerListingFailure>,
}

#[derive(Debug, Clone)]
pub struct ServerListingFailure {
    pub server: McpServerId,
    pub message: String,
}

impl ToolInventory {
    #[must_use]
    pub const fn complete(tools: Vec<ToolDefinition>) -> Self {
        Self {
            tools,
            failed_servers: Vec::new(),
        }
    }

    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.failed_servers.is_empty()
    }
}
