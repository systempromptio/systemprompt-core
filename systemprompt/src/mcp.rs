//! The MCP domain: [`host`], the YAML-declared toolset host, and with the
//! `mcp-servers` feature the server runtime, registry, tool and resource
//! abstractions of `systemprompt-mcp`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "mcp-servers")]
pub use systemprompt_mcp::*;

/// The YAML-declared MCP server host.
///
/// [`serve`](host::serve) a deployment's `toolsets:`. A member contributes
/// toolsets into the host's `ToolsetSlot` from its `contribute`.
pub mod host {
    pub use systemprompt_mcp_host::McpHostExtension;
    pub use systemprompt_mcp_host::contract::ToolsetSlot;
    pub use systemprompt_mcp_host::serving::host::*;
    pub use systemprompt_mcp_host::toolsets::*;
}
