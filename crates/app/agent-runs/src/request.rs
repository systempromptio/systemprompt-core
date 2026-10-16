//! [`RunRequest`]: everything a host needs to run one headless turn.
//!
//! The request is stored verbatim as `agent_runs.request` (JSONB), so it is
//! the contract between the product that queues a run and the executor that
//! claims it. [`HostKind`] names the coding-agent host; its spelling is the
//! `agent_runs.host` column.
//!
//! `RunRequest::attribution_scopes` is header-only attribution: every gateway
//! call the run makes carries one `x-systemprompt-scope-<dimension>: <value>`
//! header per binding (`scope_headers`), so a trace or cost rollup can key on
//! it without the run's API key being bound to the value (`workflow=<run_id>`,
//! `project=<id>`, …).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{HostSessionId, PluginId};
use systemprompt_models::attribution::ScopeBinding;

use crate::macros::text_enum;

text_enum!(
    HostKind, UnknownHost {
        ClaudeCode => "claude-code",
        CodexCli => "codex-cli",
        OpenCode => "opencode",
        Hermes => "hermes",
    }
);

text_enum!(
    PermissionMode, UnknownPermissionMode {
        Default => "default",
        AcceptEdits => "acceptEdits",
        BypassPermissions => "bypassPermissions",
        Plan => "plan",
    }
);

/// An MCP server handed to the host, keyed by its name in
/// [`RunRequest::mcp_servers`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpServerSpec {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRequest {
    pub host: HostKind,
    pub prompt: String,
    #[serde(default)]
    pub system_prompt_append: Option<String>,
    #[serde(default)]
    pub persona: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub disallowed_tools: Vec<String>,
    pub permission_mode: PermissionMode,
    #[serde(default)]
    pub permission_prompts_none: bool,
    /// The directory the host runs in, for Claude Code and Codex alike.
    /// Claude Code falls back to the run's scratch `workspace` when empty.
    pub workspace: PathBuf,
    #[serde(default)]
    pub add_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub mcp_servers: BTreeMap<String, McpServerSpec>,
    // JSON: a JSON Schema the host must make its final answer satisfy; its
    // shape is whatever the caller's output contract is.
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub max_turns: Option<u32>,
    #[serde(default)]
    pub max_budget_usd: Option<f64>,
    pub wall_clock: Duration,
    #[serde(default)]
    pub resume_session_id: Option<HostSessionId>,
    #[serde(default = "first_attempt")]
    pub attempt: u32,
    #[serde(default)]
    pub attribution_scopes: Vec<ScopeBinding>,
    #[serde(default)]
    pub plugin_id: Option<PluginId>,
    #[serde(default)]
    pub plugin_dirs: Vec<PathBuf>,
}

impl RunRequest {
    #[must_use]
    pub fn scope_headers(&self) -> BTreeMap<String, String> {
        self.attribution_scopes
            .iter()
            .map(|binding| {
                (
                    format!("x-systemprompt-scope-{}", binding.dimension.as_str()),
                    binding.value.clone(),
                )
            })
            .collect()
    }
}

const fn first_attempt() -> u32 {
    1
}
