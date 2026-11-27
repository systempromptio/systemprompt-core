//! What an engine extension hands the engine: tools, prompt sections, and the
//! wrapper that keeps each contribution with the extension it came from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use systemprompt_identifiers::engine::{ExtensionId, SessionId};
use crate::tools::Tool;

/// A tool an extension adds beside the built-ins.
///
/// It is offered after the built-ins, behind the same agent tool filter and
/// rules, under `permission`; a tool that shares a built-in's name is not
/// offered. One that `asks_itself` calls the permission gate from its own
/// `run`; any other is asked about by the engine before it runs.
#[derive(Clone)]
pub struct ExtensionTool {
    pub tool: Arc<dyn Tool>,
    pub permission: &'static str,
    pub asks_itself: bool,
}

impl ExtensionTool {
    pub const fn asking_itself(tool: Arc<dyn Tool>, permission: &'static str) -> Self {
        Self {
            tool,
            permission,
            asks_itself: true,
        }
    }

    pub const fn gated(tool: Arc<dyn Tool>, permission: &'static str) -> Self {
        Self {
            tool,
            permission,
            asks_itself: false,
        }
    }
}

impl fmt::Debug for ExtensionTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtensionTool")
            .field("tool", &self.tool.name())
            .field("permission", &self.permission)
            .field("asks_itself", &self.asks_itself)
            .finish()
    }
}

/// A block of system prompt text an extension adds to one step, sent after
/// the instruction files under its `heading`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptSection {
    pub heading: String,
    pub text: String,
}

/// The step a prompt section is asked for. `subagent` is set when the session
/// is a delegated subagent's.
#[derive(Debug, Clone, Copy)]
pub struct PromptContext<'a> {
    pub session_id: &'a SessionId,
    pub agent: &'a str,
    pub model_id: &'a str,
    pub directory: &'a Path,
    pub subagent: bool,
}

/// One contribution and the extension it came from.
#[derive(Debug, Clone)]
pub struct Contributed<T> {
    pub extension: ExtensionId,
    pub item: T,
}
