//! Projects configured managed MCP servers into the signed `ManagedMcpServer`
//! records the manifest carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_identifiers::{McpServerId, ValidatedUrl};
use systemprompt_models::bridge::ids::{ManagedMcpServerName, ToolName};
use systemprompt_models::bridge::manifest::ManagedMcpServer;
use systemprompt_models::mcp::Deployment;
use systemprompt_models::services::ServicesConfig;

use crate::error::MarketplaceError;

pub fn load_managed_mcp_servers(
    services: &ServicesConfig,
    api_external_url: &str,
) -> Result<Vec<ManagedMcpServer>, MarketplaceError> {
    let base = api_external_url.trim_end_matches('/');
    let mut entries: Vec<(&String, &Deployment)> = services
        .mcp_servers
        .iter()
        .filter(|(_, d)| d.enabled)
        .collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));

    let mut out = Vec::with_capacity(entries.len());
    for (name, deployment) in entries {
        // Why: a server with no declared tool policy has no decision the bridge
        // can enforce; it is withheld from the signed manifest rather than
        // published as allow-all, and boot validation names it.
        let Some(tool_policy) = deployment.tool_policy else {
            tracing::warn!(server = %name, "MCP server has no tool_policy and is withheld from the bridge manifest");
            continue;
        };
        // Why: a `connector:` server has its per-user accessor synthesised by
        // the registry resolver, so like an `external_auth` server it must be
        // reached through the gateway, which swaps the caller's JWT for their
        // grant. Publishing its raw endpoint sent the gateway token straight to
        // Google and every tool call came back 401.
        let url_str = if deployment.external_auth.is_some() || deployment.connector.is_some() {
            format!("{base}/api/v1/mcp/{name}/mcp")
        } else {
            match deployment.endpoint.as_deref() {
                Some(ep) if ep.starts_with("http://") || ep.starts_with("https://") => {
                    ep.to_owned()
                },
                Some(rel) if !rel.is_empty() => format!("{base}{rel}"),
                _ => format!("{base}/api/v1/mcp/{name}/mcp"),
            }
        };
        let url =
            ValidatedUrl::try_new(url_str).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        let mcp_name = ManagedMcpServerName::try_new(name.clone())
            .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        out.push(ManagedMcpServer {
            id: McpServerId::try_new(name.clone())
                .map_err(|e| MarketplaceError::Catalog(e.to_string()))?,
            name: mcp_name,
            url,
            transport: Some("http".to_owned()),
            headers: None,
            oauth: Some(deployment.oauth.required),
            tool_policy: Some(BTreeMap::from([(
                ToolName::try_new(ManagedMcpServer::TOOL_POLICY_WILDCARD)
                    .map_err(|e| MarketplaceError::Catalog(e.to_string()))?,
                tool_policy,
            )])),
        });
    }
    Ok(out)
}

pub fn disabled_mcp_server_names(services: &ServicesConfig) -> BTreeSet<String> {
    services
        .mcp_servers
        .iter()
        .filter(|(_, d)| !d.enabled)
        .map(|(name, _)| name.clone())
        .collect()
}
