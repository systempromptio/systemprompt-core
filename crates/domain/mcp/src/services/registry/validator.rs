use anyhow::Result;
use std::collections::HashSet;
use systemprompt_models::mcp::RegistryConfig;

pub fn validate_registry(config: &RegistryConfig) -> Result<()> {
    tracing::info!("Validating registry configuration");

    validate_port_conflicts(config)?;
    validate_server_configs(config)?;
    validate_oauth_configs(config)?;

    tracing::info!("Registry validation passed");
    Ok(())
}

fn validate_port_conflicts(config: &RegistryConfig) -> Result<()> {
    let mut seen_ports = HashSet::new();

    let conflicts: Vec<_> = config
        .servers
        .iter()
        .filter(|s| s.enabled)
        .filter(|s| !seen_ports.insert(s.port))
        .map(|s| format!("{}:{}", s.name, s.port))
        .collect();

    if conflicts.is_empty() {
        tracing::debug!(
            enabled_servers = seen_ports.len(),
            "No port conflicts found"
        );
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "Port conflicts detected: {}",
        conflicts.join(", ")
    ))
}

fn validate_server_configs(config: &RegistryConfig) -> Result<()> {
    let invalid_servers: Vec<String> = config
        .servers
        .iter()
        .filter(|s| s.enabled)
        .flat_map(validate_single_server)
        .collect();

    if invalid_servers.is_empty() {
        tracing::debug!("All server configurations valid");
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "Invalid server configurations:\n{}",
        invalid_servers.join("\n")
    ))
}

fn validate_single_server(
    server_config: &systemprompt_models::mcp::McpServerConfig,
) -> Vec<String> {
    let mut errors = Vec::new();
    let name = &server_config.name;

    if server_config.port < 1024 {
        errors.push(format!("{name}: invalid port {}", server_config.port));
        return errors;
    }

    if !server_config.crate_path.exists() {
        errors.push(format!(
            "{name}: crate path does not exist: {}",
            server_config.crate_path.display()
        ));
        return errors;
    }

    if server_config.display_name.is_empty() {
        errors.push(format!("{name}: missing display_name"));
    }

    if server_config.description.is_empty() {
        errors.push(format!("{name}: missing description"));
    }

    errors
}

fn validate_oauth_configs(config: &RegistryConfig) -> Result<()> {
    let oauth_issues: Vec<_> = config
        .servers
        .iter()
        .filter(|s| s.enabled && s.oauth.required && s.oauth.scopes.is_empty())
        .map(|s| format!("{}: OAuth enabled but no scopes defined", s.name))
        .collect();

    if oauth_issues.is_empty() {
        tracing::debug!("OAuth configurations valid");
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "OAuth configuration issues:\n{}",
        oauth_issues.join("\n")
    ))
}
