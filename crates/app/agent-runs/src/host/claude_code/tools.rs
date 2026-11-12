//! The Claude Code tool set a run's declared tools restrict it to.
//!
//! `RunRequest::allowed_tools` entries are permission-rule patterns
//! (`Read`, `Bash(git:*)`, `mcp__server`, `mcp__server__tool`). A non-empty
//! list is the set of tools available to the run: the built-in base names go
//! to `--tools` in every permission mode, and the patterns themselves still go
//! to `--allowedTools` as pre-approvals. `--tools` governs only the built-in
//! set; outside bypass mode a headless run cannot call an MCP tool that
//! `--allowedTools` does not pre-approve, so per-tool MCP entries are enforced
//! by the host. Under bypass that gate is gone, so an MCP server is admitted
//! whole or not at all: every configured server must be declared as a whole
//! server, and a per-tool MCP entry is refused. In every mode an entry naming
//! an unconfigured MCP server, or one that is not a pattern, is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use super::args::bypasses_permissions;
use crate::error::{AgentRunsError, AgentRunsResult};
use crate::request::{HostKind, RunRequest};

const MCP_PREFIX: &str = "mcp__";

pub fn restricted_builtins(request: &RunRequest) -> AgentRunsResult<Option<Vec<String>>> {
    if request.allowed_tools.is_empty() {
        return Ok(None);
    }
    let mut builtins: Vec<String> = Vec::new();
    let bypass = bypasses_permissions(request);
    let mut servers: BTreeSet<&str> = BTreeSet::new();
    for entry in &request.allowed_tools {
        let entry = entry.trim();
        if let Some(rest) = entry.strip_prefix(MCP_PREFIX) {
            servers.insert(mcp_server(entry, rest, request, bypass)?);
            continue;
        }
        let base = entry.split('(').next().unwrap_or_default().trim();
        if base.is_empty() || !base.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(refused(entry, "not a tool name or permission-rule pattern"));
        }
        if !builtins.iter().any(|known| known == base) {
            builtins.push(base.to_owned());
        }
    }
    if let Some(undeclared) = request
        .mcp_servers
        .keys()
        .find(|name| bypass && !servers.contains(name.as_str()))
    {
        return Err(refused(
            undeclared,
            "configured MCP server is not among the declared tools",
        ));
    }
    Ok(Some(builtins))
}

fn mcp_server<'a>(
    entry: &str,
    rest: &'a str,
    request: &RunRequest,
    bypass: bool,
) -> AgentRunsResult<&'a str> {
    let (server, tool) = rest.split_once("__").unwrap_or((rest, "*"));
    if bypass && tool != "*" {
        return Err(refused(
            entry,
            "under bypassed permissions a single MCP tool cannot be isolated from its server; \
             declare the whole server",
        ));
    }
    if !request.mcp_servers.contains_key(server) {
        return Err(refused(
            entry,
            "names an MCP server the run does not configure",
        ));
    }
    Ok(server)
}

fn refused(entry: &str, reason: &'static str) -> AgentRunsError {
    AgentRunsError::ToolPolicyUnenforceable {
        host: HostKind::ClaudeCode,
        entry: entry.to_owned(),
        reason,
    }
}
