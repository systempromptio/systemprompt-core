//! The brain's MCP toolset as a provided value.
//!
//! The toolset host cannot name the brain's crate, so the brain member
//! provides a [`BrainToolsetSource`] and the host's `brain` toolset factory
//! builds from it. A source answers `None` while the profile has the brain
//! off, so a server that lists the toolset still boots and offers no brain
//! tools.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::McpServerId;

use crate::mcp::McpToolset;

pub const BRAIN_TOOLSET: &str = "brain";

/// Builds the brain's agent toolset for one hosted MCP server.
pub trait BrainToolsetSource: Send + Sync {
    fn toolset(&self, server: &McpServerId) -> Option<Box<dyn McpToolset>>;
}

pub type DynBrainToolsetSource = Arc<dyn BrainToolsetSource>;
