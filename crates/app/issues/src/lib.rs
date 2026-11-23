//! `systemprompt-issues` — issue reports.
//!
//! [`mcp`] is the MCP tool library a server mounts so a user's agent files
//! and follows bug and feedback reports. With `cli`, [`commands`] is the
//! `admin issues` triage group the `issues` member
//! ([`extension::IssuesExtension`]) contributes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// The `admin issues` triage commands.
#[cfg(feature = "cli")]
pub mod commands;
/// The issues member.
pub mod extension;
/// The issue report MCP tools.
pub mod mcp;
