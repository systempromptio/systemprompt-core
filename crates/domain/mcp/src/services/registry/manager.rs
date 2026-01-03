use anyhow::Result;
use std::path::Path;
use systemprompt_loader::ExtensionRegistry;
use systemprompt_models::Config;

use crate::services::deployment::DeploymentService;

#[derive(Debug, Clone, Copy)]
pub struct RegistryService;

impl RegistryService {
    pub fn get_enabled_servers_as_config() -> Result<Vec<crate::McpServerConfig>> {
        use systemprompt_loader::ConfigLoader;

        let global_config = Config::get()?;
        let registry = ExtensionRegistry::build(
            Path::new(&global_config.system_path),
            global_config.is_cloud,
            &global_config.bin_path,
        );
        let services_config = ConfigLoader::load()?;
        let mut enabled = Vec::new();

        let mut server_names: Vec<_> = services_config.mcp_servers.keys().collect();
        server_names.sort();

        for server_name in server_names {
            let deployment = &services_config.mcp_servers[server_name];
            if !deployment.enabled {
                continue;
            }

            let crate_path = registry.get_path(&deployment.binary)?;
            let display_name = deployment
                .package
                .clone()
                .unwrap_or_else(|| deployment.binary.clone());

            let config = crate::McpServerConfig {
                name: server_name.clone(),
                binary: deployment.binary.clone(),
                enabled: deployment.enabled,
                display_in_web: deployment.display_in_web,
                port: deployment.port,
                crate_path,
                display_name: display_name.clone(),
                description: format!("{display_name} MCP Server"),
                capabilities: vec![],
                schemas: deployment.schemas.clone(),
                oauth: deployment.oauth.clone(),
                tools: deployment.tools.clone(),
                model_config: deployment.model_config.clone(),
                env_vars: deployment.env_vars.clone(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                host: "0.0.0.0".to_string(),
                module_name: "mcp".to_string(),
                protocol: "mcp".to_string(),
            };
            enabled.push(config);
        }

        Ok(enabled)
    }

    pub fn validate_registry() -> Result<()> {
        DeploymentService::validate_config()?;
        Ok(())
    }
}
