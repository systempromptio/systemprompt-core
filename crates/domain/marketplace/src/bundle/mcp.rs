//! MCP component projection: a plugin's referenced managed servers laid out as
//! the host's `.mcp.json`.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use systemprompt_models::bridge::manifest::ManagedMcpServer;
use systemprompt_models::services::PluginConfig;

use super::{BundleFile, PluginBundle};
use crate::error::MarketplaceError;

/// How a plugin's `mcp_servers.include` entry resolves against the catalogue.
enum McpReference<'a> {
    /// Defined and `enabled: true` — projected into `.mcp.json`.
    Enabled(&'a ManagedMcpServer),
    /// Defined but `enabled: false` — omitted until re-enabled, no warning.
    Disabled,
    /// Not defined in `services.mcp_servers`; a genuine misconfiguration.
    /// Unreachable once `validate_plugin_bindings` has run, kept defensively.
    Unknown,
}

fn classify<'a>(
    name: &str,
    enabled: &'a [ManagedMcpServer],
    disabled: &BTreeSet<String>,
) -> McpReference<'a> {
    if let Some(server) = enabled.iter().find(|s| s.name.as_str() == name) {
        McpReference::Enabled(server)
    } else if disabled.contains(name) {
        McpReference::Disabled
    } else {
        McpReference::Unknown
    }
}

#[derive(Serialize)]
struct McpConfigFile {
    #[serde(rename = "mcpServers")]
    mcp_servers: BTreeMap<String, McpServerEntry>,
}

#[derive(Serialize)]
struct McpServerEntry {
    #[serde(rename = "type")]
    server_type: String,
    url: String,
}

pub(super) fn append_mcp_file(
    config: &PluginConfig,
    servers: &[ManagedMcpServer],
    disabled: &BTreeSet<String>,
    bundle: &mut PluginBundle,
) -> Result<(), MarketplaceError> {
    if config.mcp_servers.include.is_empty() {
        return Ok(());
    }

    let mut entries = BTreeMap::new();
    for name in &config.mcp_servers.include {
        match classify(name.as_str(), servers, disabled) {
            McpReference::Enabled(server) => {
                entries.insert(
                    name.clone(),
                    McpServerEntry {
                        server_type: "http".to_owned(),
                        url: server.url.as_str().to_owned(),
                    },
                );
            },
            McpReference::Disabled => {
                tracing::debug!(
                    plugin_id = %config.id,
                    mcp_server = %name,
                    "bundle: referenced MCP server is disabled; omitting from bundle"
                );
            },
            McpReference::Unknown => {
                tracing::warn!(
                    plugin_id = %config.id,
                    mcp_server = %name,
                    "bundle: referenced MCP server not found in catalogue; skipping"
                );
            },
        }
    }

    if entries.is_empty() {
        return Ok(());
    }

    let json = serde_json::to_vec_pretty(&McpConfigFile {
        mcp_servers: entries,
    })
    .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    bundle.insert(
        ".mcp.json".to_owned(),
        BundleFile {
            bytes: json,
            executable: false,
        },
    );
    Ok(())
}
