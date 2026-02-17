use anyhow::Result;
use std::path::Path;
use systemprompt_models::PluginConfig;

pub fn generate_mcp_json(
    plugin: &PluginConfig,
    services_path: &Path,
    output_dir: &Path,
    files_generated: &mut Vec<String>,
) -> Result<()> {
    if plugin.mcp_servers.is_empty() {
        return Ok(());
    }

    let config_path = services_path.join("config").join("config.yaml");
    let mut mcp_servers = serde_json::Map::new();

    for mcp_name in &plugin.mcp_servers {
        let port = resolve_mcp_port(mcp_name, &config_path).unwrap_or(5000);
        let url = format!("http://localhost:{}/api/v1/mcp/{}/mcp", port, mcp_name);
        let mut server = serde_json::Map::new();
        server.insert("url".to_string(), serde_json::Value::String(url));
        mcp_servers.insert(mcp_name.clone(), serde_json::Value::Object(server));
    }

    let mcp_json = serde_json::json!({ "mcpServers": mcp_servers });
    let mcp_path = output_dir.join(".mcp.json");
    let content = serde_json::to_string_pretty(&mcp_json)?;
    std::fs::write(&mcp_path, content)?;
    files_generated.push(mcp_path.to_string_lossy().to_string());

    Ok(())
}

fn resolve_mcp_port(mcp_name: &str, config_path: &Path) -> Option<u16> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| {
            tracing::debug!(error = %e, path = %config_path.display(), "Failed to read config for MCP port resolution");
            e
        })
        .ok()?;
    let config: serde_yaml::Value = serde_yaml::from_str(&content)
        .map_err(|e| {
            tracing::warn!(error = %e, path = %config_path.display(), "Failed to parse config for MCP port resolution");
            e
        })
        .ok()?;
    config
        .get("mcp_servers")
        .and_then(|m| m.get(mcp_name))
        .and_then(|s| s.get("port"))
        .and_then(serde_yaml::Value::as_u64)
        .and_then(|p| u16::try_from(p).ok())
}
