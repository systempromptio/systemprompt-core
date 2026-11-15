//! The toolset factory shape.
//!
//! A crate that ships an [`McpToolset`] contributes how to build it from the
//! server's [`HostContext`] through the
//! [`ToolsetSlot`](crate::contract::ToolsetSlot); a deployment hosts it by
//! listing its id under `toolsets:`. `Ok(None)` from a factory means the
//! toolset is unavailable in this installation and is skipped with a warning;
//! an `Err` fails start.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_mcp::McpToolset;

pub type ToolsetBuildError = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type ToolsetBuildResult = Result<Option<Box<dyn McpToolset>>, ToolsetBuildError>;
