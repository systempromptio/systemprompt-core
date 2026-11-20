//! `systemprompt-mcp-entry` — the MCP entry surface.
//!
//! The `http` feature carries the capability's route: the MCP server
//! registry at `/api/v1/mcp/registry`.
//! `extension::McpHttpExtension`, built with it, contributes the route to the
//! composition, so the API server does not name this crate. The `plugins mcp`
//! commands are the CLI kernel's, so the `cli` feature carries no command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "http")]
pub mod extension;
#[cfg(feature = "http")]
pub mod http;
