//! Claude Desktop's per-tool `toolPolicy`.
//!
//! The manifest's decision for a managed server, expanded over the tool names
//! the server last reported. Desktop spells the decisions `allow` / `ask` /
//! `blocked`. The manifest wildcard applies to every name the tool catalog
//! knows for that server, and a named tool's own entry wins over the wildcard.
//! A wildcard `deny` is not expanded: it withholds the server from the policy
//! (`denied_outright`), because a name the catalog has not seen would
//! otherwise fall back to asking.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

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

#[must_use]
pub fn denied_outright(upstream: &crate::mcp_registry::McpUpstream) -> bool {
    use systemprompt_models::bridge::ids::ToolPolicy;
    use systemprompt_models::bridge::manifest::ManagedMcpServer;
    upstream
        .tool_policy
        .get(ManagedMcpServer::TOOL_POLICY_WILDCARD)
        .is_some_and(|policy| *policy == ToolPolicy::Deny)
}

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
