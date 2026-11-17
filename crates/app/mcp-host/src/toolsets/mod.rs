//! The YAML-declared MCP server host.
//!
//! A deployment that lists `toolsets:` and names no `binary:` needs no
//! hand-written server: core re-runs its own executable as
//! `admin mcp serve --server <id>`, which boots a
//! [`HostContext`](crate::HostContext) from its environment and calls
//! [`serve`]. [`serve`] builds the listed toolsets from the built
//! [`Toolsets`](crate::contract::Toolsets), filters `tools/list` per caller,
//! gates each call on the tool's resolved access and audits every outcome
//! through the contributed access recorders. A separate extension binary that
//! hosts toolsets calls [`serve`] the same way.
//! Core contributes the `issues` and `documents` toolsets; an extension adds
//! its own through the [`ToolsetSlot`](crate::contract::ToolsetSlot).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod core_toolsets;
mod factory;
mod serve;

pub use core_toolsets::{
    DOCUMENTS_TOOLSET, DocumentsMcpError, ISSUES_TOOLSET, IssuesMcpError, documents_tools,
    issues_tools,
};
pub use factory::{ToolsetBuildError, ToolsetBuildResult};
pub use serve::{McpHostError, build_server, serve, serve_on};
