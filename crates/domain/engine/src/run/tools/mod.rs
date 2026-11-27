//! Tools as the engine runs them: the per-agent registry, the permission gate,
//! the engine's own tools (task, skill, question), the MCP adapter, and the
//! hooks tools call back into.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod asks;
pub(crate) mod exec;
pub(crate) mod gate;
pub(crate) mod hooks;
pub(crate) mod mcp;
pub(crate) mod outcome;
pub(crate) mod progress;
pub(crate) mod question;
pub(crate) mod registry;
pub(crate) mod skill;
pub(crate) mod task;
