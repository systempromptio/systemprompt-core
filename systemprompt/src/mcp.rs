//! The MCP domain: server runtime, registry, tool and resource abstractions,
//! and [`host`], the YAML-declared toolset host.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub use systemprompt_mcp::{
    register_artifact_theme, register_mcp_access_recorder, register_ui_renderer, *,
};

/// The YAML-declared MCP server host: [`serve`] a deployment's
/// `toolsets:`, register toolsets with [`submit_toolset!`] and access
/// recorders with [`register_mcp_access_recorder!`].
pub mod host {
    pub use systemprompt_mcp::host::*;
    pub use systemprompt_mcp::register_mcp_access_recorder;
    pub use systemprompt_runtime::mcp_host::*;
    pub use systemprompt_runtime::submit_toolset;
}
