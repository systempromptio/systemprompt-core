use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_loader::ExtensionRegistry;
use systemprompt_models::Config;

use crate::services::deployment::DeploymentService;
use crate::ServerManifest;

#[derive(Debug, Clone, Copy)]
pub struct RegistryService;

impl RegistryService {
    fn registry_dir() -> PathBuf {
        Config::get().map_or_else(
            |_| PathBuf::from("/tmp/systemprompt/metadata/mcp"),
            |config| PathBuf::from(&config.system_path).join("metadata/mcp"),
        )
    }

    pub fn load_manifest(name: &str) -> Result<ServerManifest> {
        let registry_dir = Self::registry_dir();
        let path = registry_dir.join(format!("{name}.json"));

        let content = fs::read_to_string(&path)
            .map_err(|e| anyhow!("Failed to read manifest file {}: {}", path.display(), e))?;

        let manifest: ServerManifest = serde_json::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse manifest JSON {}: {}", path.display(), e))?;

        Ok(manifest)
    }

    pub fn list_servers() -> Vec<String> {
        let registry_dir = Self::registry_dir();

        let Ok(entries) = fs::read_dir(&registry_dir) else {
            return Vec::new();
        };

        entries
            .filter_map(|entry| {
                entry.ok().and_then(|e| {
                    let path = e.path();
                    if path.extension()?.to_str()? == "json" {
                        path.file_stem()?.to_str().map(ToString::to_string)
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    pub fn get_enabled_servers() -> Result<Vec<(ServerManifest, u16)>> {
        let servers = Self::list_servers();
        let mut enabled = Vec::new();

        for server_name in servers {
            if DeploymentService::is_server_enabled(&server_name)? {
                let manifest = Self::load_manifest(&server_name)?;
                let port = DeploymentService::get_server_port(&server_name)?;
                enabled.push((manifest, port));
            }
        }

        Ok(enabled)
    }

    pub fn get_enabled_servers_as_config() -> Result<Vec<crate::McpServerConfig>> {
        use systemprompt_loader::ConfigLoader;

        let global_config = Config::get()?;
        let registry = ExtensionRegistry::build(Path::new(&global_config.system_path));
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

    pub fn get_server_with_deployment(name: &str) -> Result<(ServerManifest, u16)> {
        let manifest = Self::load_manifest(name)?;
        let port = DeploymentService::get_server_port(name)?;
        Ok((manifest, port))
    }

    pub fn validate_registry() -> Result<()> {
        DeploymentService::validate_config()?;
        Ok(())
    }
}

pub fn get_enabled_servers(
    _config: &systemprompt_models::ServicesConfig,
) -> Result<Vec<crate::McpServerConfig>> {
    RegistryService::get_enabled_servers_as_config()
}

pub fn get_all_servers(
    config: &systemprompt_models::ServicesConfig,
) -> Result<Vec<crate::McpServerConfig>> {
    let global_config = Config::get()?;
    let registry = ExtensionRegistry::build(Path::new(&global_config.system_path));
    let servers = RegistryService::list_servers();
    let mut configs = Vec::new();

    for server_name in servers {
        let manifest = RegistryService::load_manifest(&server_name)?;
        let deployment = config
            .mcp_servers
            .get(&server_name)
            .ok_or_else(|| anyhow!("No deployment config for {server_name}"))?;

        let crate_path = registry.get_path(&deployment.binary)?;

        let server_config = crate::McpServerConfig::from_manifest_and_deployment(
            server_name,
            &manifest,
            deployment,
            crate_path,
        );
        configs.push(server_config);
    }

    Ok(configs)
}

pub fn get_server_by_name(
    config: &systemprompt_models::ServicesConfig,
    name: &str,
) -> Result<Option<crate::McpServerConfig>> {
    if let Some(deployment) = config.mcp_servers.get(name) {
        let global_config = Config::get()?;
        let registry = ExtensionRegistry::build(Path::new(&global_config.system_path));
        let manifest = RegistryService::load_manifest(name)?;

        let crate_path = registry.get_path(&deployment.binary)?;

        let server_config = crate::McpServerConfig::from_manifest_and_deployment(
            name.to_string(),
            &manifest,
            deployment,
            crate_path,
        );
        Ok(Some(server_config))
    } else {
        Ok(None)
    }
}

pub fn count_enabled_servers(config: &systemprompt_models::ServicesConfig) -> usize {
    config.mcp_servers.values().filter(|d| d.enabled).count()
}

pub fn get_servers_by_oauth_requirement(
    config: &systemprompt_models::ServicesConfig,
    oauth_required: bool,
) -> Result<Vec<crate::McpServerConfig>> {
    let global_config = Config::get()?;
    let registry = ExtensionRegistry::build(Path::new(&global_config.system_path));
    let mut configs = Vec::new();

    for (server_name, deployment) in &config.mcp_servers {
        if deployment.enabled && deployment.oauth.required == oauth_required {
            let manifest = RegistryService::load_manifest(server_name)?;

            let crate_path = registry.get_path(&deployment.binary)?;

            let server_config = crate::McpServerConfig::from_manifest_and_deployment(
                server_name.clone(),
                &manifest,
                deployment,
                crate_path,
            );
            configs.push(server_config);
        }
    }

    Ok(configs)
}
