//! The YAML-declared MCP server host.
//!
//! A deployment that lists `toolsets:` needs no hand-written server: its
//! binary calls [`serve`] with the application context and its server id, and
//! core builds the listed toolsets from the [`ToolsetFactory`] inventory,
//! filters `tools/list` per caller, gates each call on the tool's resolved
//! access and audits every outcome through the registered recorders.
//! Core registers the `issues` and `documents` toolsets; an extension adds its
//! own with [`submit_toolset!`](crate::submit_toolset).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod core_toolsets;
mod factory;
mod serve;

pub use core_toolsets::{DOCUMENTS_TOOLSET, ISSUES_TOOLSET};
pub use factory::{
    ToolsetBuildError, ToolsetBuildResult, ToolsetFactory, find_toolset_factory,
    registered_toolset_ids, toolset_factories,
};
pub use serve::{McpHostError, build_server, serve, serve_on};
