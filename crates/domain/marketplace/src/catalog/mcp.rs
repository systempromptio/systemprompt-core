//! Projects configured managed MCP servers into the signed `ManagedMcpServer`
//! records the manifest carries.

use std::collections::BTreeSet;

use systemprompt_identifiers::ValidatedUrl;
use systemprompt_models::bridge::ids::ManagedMcpServerName;
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
        // An accessor-backed external server is proxied through the gateway so
        // its provider URL and per-user token never reach the client.
        let url_str = if deployment.external_auth.is_some() {
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
            name: mcp_name,
            url,
            transport: Some("http".to_owned()),
            headers: None,
            oauth: Some(deployment.oauth.required),
            tool_policy: None,
        });
    }
    Ok(out)
}

/// Names defined in `services.mcp_servers` with `enabled: false`.
///
/// Validation accepts a plugin reference to a defined-but-disabled server by
/// design, so such a server is absent from the enabled catalogue without being
/// a misconfiguration. The bundle builder consults this set to tell that quiet,
/// temporary omission apart from a genuinely unknown reference.
pub fn disabled_mcp_server_names(services: &ServicesConfig) -> BTreeSet<String> {
    services
        .mcp_servers
        .iter()
        .filter(|(_, d)| !d.enabled)
        .map(|(name, _)| name.clone())
        .collect()
}
