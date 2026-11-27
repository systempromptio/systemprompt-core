//! Settings files: the global file, then each project level from the worktree
//! root down to the working directory.
//!
//! Objects merge key by key with nearer files winning. Permission rules are not
//! merged as objects: each file's rules are appended in file order so that
//! last-match-wins keeps meaning "the nearest file decides". Instruction
//! lists are concatenated. Keys this crate does not know are kept in
//! [`Settings::extra`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod load;
mod raw;

use std::fmt;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::agent::AgentOverride;
use crate::config::permission::Ruleset;

pub(crate) use load::{load, validate_text};

/// The merged settings for one working directory.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    pub model: Option<String>,
    pub small_model: Option<String>,
    pub default_agent: Option<String>,
    pub agents: IndexMap<String, AgentOverride>,
    pub permission: Ruleset,
    pub mcp: IndexMap<String, McpServer>,
    pub lsp: IndexMap<String, LspServer>,
    pub formatters: IndexMap<String, Formatter>,
    pub shell: Option<String>,
    pub instructions: Vec<String>,
    pub compaction: Compaction,
    // JSON: experimental switches are free-form values the engine reads by name.
    pub experimental: IndexMap<String, Value>,
    // JSON: settings keys this crate does not interpret, kept as written.
    pub extra: Map<String, Value>,
}

impl Settings {
    pub fn experimental_flag(&self, name: &str) -> bool {
        self.experimental
            .get(name)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub fn enabled_mcp(&self) -> impl Iterator<Item = (&String, &McpServer)> {
        self.mcp.iter().filter(|(_, server)| server.enabled())
    }
}

/// A user-defined MCP server. Header and environment values may hold
/// credentials, so `Debug` shows names only.
#[derive(Clone, PartialEq, Eq)]
pub enum McpServer {
    Remote {
        url: String,
        headers: IndexMap<String, String>,
        enabled: bool,
        timeout_ms: Option<u64>,
    },
    Local {
        command: String,
        args: Vec<String>,
        env: IndexMap<String, String>,
        enabled: bool,
        timeout_ms: Option<u64>,
    },
}

impl McpServer {
    pub const fn enabled(&self) -> bool {
        match self {
            Self::Remote { enabled, .. } | Self::Local { enabled, .. } => *enabled,
        }
    }

    pub const fn timeout_ms(&self) -> Option<u64> {
        match self {
            Self::Remote { timeout_ms, .. } | Self::Local { timeout_ms, .. } => *timeout_ms,
        }
    }
}

impl fmt::Debug for McpServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Remote {
                url,
                headers,
                enabled,
                timeout_ms,
            } => f
                .debug_struct("Remote")
                .field("url", url)
                .field("headers", &headers.keys().collect::<Vec<_>>())
                .field("enabled", enabled)
                .field("timeout_ms", timeout_ms)
                .finish(),
            Self::Local {
                command,
                args,
                env,
                enabled,
                timeout_ms,
            } => f
                .debug_struct("Local")
                .field("command", command)
                .field("args", args)
                .field("env", &env.keys().collect::<Vec<_>>())
                .field("enabled", enabled)
                .field("timeout_ms", timeout_ms)
                .finish(),
        }
    }
}

/// A language server the user configured.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LspServer {
    pub command: String,
    pub args: Vec<String>,
    pub extensions: Vec<String>,
    pub root_markers: Vec<String>,
    pub env: IndexMap<String, String>,
    pub disabled: bool,
    // JSON: LSP `initializationOptions`, passed to the server verbatim.
    pub initialization: Option<Value>,
}

/// A formatter the user configured; `args` may contain `$FILE` for the file
/// being formatted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Formatter {
    pub command: String,
    pub args: Vec<String>,
    pub extensions: Vec<String>,
    pub env: IndexMap<String, String>,
    pub disabled: bool,
}

/// When and how much of a session's history is compacted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Compaction {
    pub auto: bool,
    pub prune: bool,
    pub reserved_tokens: Option<u32>,
}

impl Default for Compaction {
    fn default() -> Self {
        Self {
            auto: true,
            prune: true,
            reserved_tokens: None,
        }
    }
}
