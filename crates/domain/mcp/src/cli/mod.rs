//! CLI integration surface for the MCP domain.
//!
//! Re-exports the command functions that the `systemprompt` CLI invokes to
//! start, stop, and inspect managed MCP servers.

mod commands;

pub use commands::*;
