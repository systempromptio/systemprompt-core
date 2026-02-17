use anyhow::{Context, Result};
use clap::Args;
use std::path::Path;

use crate::shared::CommandResult;
use crate::CliConfig;

use super::types::{PluginListOutput, PluginSummary};

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs {
    #[arg(long, help = "Show only enabled plugins")]
    pub enabled: bool,

    #[arg(long, help = "Show only disabled plugins", conflicts_with = "enabled")]
    pub disabled: bool,
}

pub fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<PluginListOutput>> {
    let plugins_path = get_plugins_path()?;
    let plugins = scan_plugins(&plugins_path)?;

    let filtered: Vec<PluginSummary> = plugins
        .into_iter()
        .filter(|p| {
            if args.enabled {
                p.enabled
            } else if args.disabled {
                !p.enabled
            } else {
                true
            }
        })
        .collect();

    let output = PluginListOutput { plugins: filtered };

    Ok(CommandResult::table(output)
        .with_title("Plugins")
        .with_columns(vec![
            "id".to_string(),
            "name".to_string(),
            "version".to_string(),
            "enabled".to_string(),
            "skill_count".to_string(),
            "agent_count".to_string(),
        ]))
}

fn get_plugins_path() -> Result<std::path::PathBuf> {
    let profile = systemprompt_models::ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.plugins()))
}

fn scan_plugins(plugins_path: &Path) -> Result<Vec<PluginSummary>> {
    if !plugins_path.exists() {
        return Ok(Vec::new());
    }

    let mut plugins = Vec::new();

    for entry in std::fs::read_dir(plugins_path)? {
        let entry = entry?;
        let plugin_path = entry.path();

        if !plugin_path.is_dir() {
            continue;
        }

        let config_path = plugin_path.join("config.yaml");
        if !config_path.exists() {
            continue;
        }

        match parse_plugin_config(&config_path) {
            Ok(plugin_file) => {
                let plugin = &plugin_file.plugin;
                let skill_count = estimate_component_count(&plugin.skills);
                let agent_count = estimate_component_count(&plugin.agents);

                plugins.push(PluginSummary {
                    id: plugin.id.clone(),
                    name: plugin.name.clone(),
                    description: plugin.description.clone(),
                    version: plugin.version.clone(),
                    enabled: plugin.enabled,
                    skill_count,
                    agent_count,
                });
            },
            Err(e) => {
                tracing::warn!(
                    path = %config_path.display(),
                    error = %e,
                    "Failed to parse plugin config"
                );
            },
        }
    }

    plugins.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(plugins)
}

fn parse_plugin_config(config_path: &Path) -> Result<systemprompt_models::PluginConfigFile> {
    let content = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let plugin_file: systemprompt_models::PluginConfigFile = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", config_path.display()))?;
    Ok(plugin_file)
}

fn estimate_component_count(component: &systemprompt_models::PluginComponentRef) -> usize {
    if component.source == systemprompt_models::ComponentSource::Explicit {
        component.include.len()
    } else {
        0
    }
}
