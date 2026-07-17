//! CLI integration surface for the MCP domain.
//!
//! Re-exports the command functions that the `systemprompt` CLI invokes to
//! start, stop, and inspect managed MCP servers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod commands;

pub use commands::*;
