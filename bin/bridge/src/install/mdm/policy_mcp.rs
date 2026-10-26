//! Projection of the managed MCP registry into Claude Desktop's
//! `managedMcpServers` policy entries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::policy::McpServerEntry;

// Why: Desktop's `toolPolicy` names tools one by one, so a wildcard can only
// be expressed over names the catalog knows. A server denied outright or one
// the manifest gave no tool policy is withheld from the list; a wildcard the
// catalog cannot expand withholds the whole list (`None`), because a partial
// map is one Desktop would resolve to its own default.
pub fn mcp_entries(
    loopback: &crate::proxy::LoopbackEndpoint,
    registry: &crate::mcp_registry::McpRegistry,
) -> std::io::Result<Option<Vec<McpServerEntry>>> {
    if registry.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let catalog = super::tool_catalog::read()?;
    Ok(mcp_entries_with(loopback, registry, &catalog))
}

pub fn mcp_entries_with(
    loopback: &crate::proxy::LoopbackEndpoint,
    registry: &crate::mcp_registry::McpRegistry,
    catalog: &super::tool_catalog::ToolCatalog,
) -> Option<Vec<McpServerEntry>> {
    if registry.is_empty() {
        return Some(Vec::new());
    }
    let mut slugs: Vec<&String> = registry.keys().collect();
    slugs.sort();
    let mut out = Vec::with_capacity(slugs.len());
    let mut unexpanded = false;
    for slug in slugs {
        let Some(upstream) = registry.get(slug) else {
            continue;
        };
        if upstream.tool_policy.is_empty() {
            tracing::warn!(target: "bridge::mdm", slug = %slug, "managed MCP server has no tool policy; withheld from the desktop policy");
            continue;
        }
        if super::desktop_tool_policy::denied_outright(upstream) {
            continue;
        }
        let Some(tool_policy) = super::desktop_tool_policy::desktop_tool_policy_map(
            upstream,
            catalog.get(slug).map_or(&[][..], Vec::as_slice),
        ) else {
            tracing::warn!(target: "bridge::mdm", slug = %slug, "tool catalog has no names for a wildcard tool policy; this server skipped, the rest still written");
            unexpanded = true;
            continue;
        };
        out.push(McpServerEntry {
            name: slug.clone(),
            url: loopback.mcp_url(slug.as_str()),
            tool_policy,
        });
    }
    // Why: an empty managedMcpServers locks Claude Desktop out of every MCP
    // server; omitting the key leaves Desktop on its own default, and the
    // policy writers clear any value an older install left (`policy_apply`).
    if unexpanded && out.is_empty() {
        return None;
    }
    Some(out)
}
