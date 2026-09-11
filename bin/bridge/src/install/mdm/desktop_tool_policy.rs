//! Claude Desktop's per-tool `toolPolicy`: the manifest's decision for a
//! managed server, expanded over the tool names the server last reported.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

/// Claude Desktop's spelling of a manifest decision.
#[must_use]
pub const fn desktop_tool_policy(
    policy: systemprompt_models::bridge::ids::ToolPolicy,
) -> &'static str {
    use systemprompt_models::bridge::ids::ToolPolicy;
    match policy {
        ToolPolicy::Allow => "allow",
        ToolPolicy::Prompt => "ask",
        ToolPolicy::Deny => "blocked",
    }
}

/// Expands a server's manifest decisions into Desktop's per-tool map: the
/// wildcard applies to every name the tool catalog knows for that server,
/// and a named tool's own entry wins over the wildcard.
#[must_use]
pub fn desktop_tool_policy_map(
    upstream: &crate::mcp_registry::McpUpstream,
    known_tools: &[String],
) -> BTreeMap<String, String> {
    use systemprompt_models::bridge::manifest::ManagedMcpServer;
    let mut out = BTreeMap::new();
    if let Some(wildcard) = upstream
        .tool_policy
        .get(ManagedMcpServer::TOOL_POLICY_WILDCARD)
    {
        for tool in known_tools {
            out.insert(tool.clone(), desktop_tool_policy(*wildcard).to_owned());
        }
    }
    for (tool, policy) in &upstream.tool_policy {
        if tool != ManagedMcpServer::TOOL_POLICY_WILDCARD {
            out.insert(tool.clone(), desktop_tool_policy(*policy).to_owned());
        }
    }
    out
}
