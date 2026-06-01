//! Projects configured managed MCP servers into the signed `ManagedMcpServer`
//! records the manifest carries.

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
        let url_str = match deployment.endpoint.as_deref() {
            Some(ep) if ep.starts_with("http://") || ep.starts_with("https://") => ep.to_owned(),
            Some(rel) if !rel.is_empty() => format!("{base}{rel}"),
            _ => format!("{base}/api/v1/mcp/{name}/mcp"),
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
