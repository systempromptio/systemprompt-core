//! `systemprompt-mcp-entry` — the MCP entry surface.
//!
//! The `http` feature carries the capability's route: the MCP server
//! registry at `/api/v1/mcp/registry`. The `cli` feature carries the
//! `plugins mcp` commands. [`extension::McpHttpExtension`] contributes both
//! to the composition, so neither the API server nor the CLI names this
//! crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "cli")]
pub mod cli;
pub mod extension;
#[cfg(feature = "http")]
pub mod http;
