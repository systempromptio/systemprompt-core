use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::path::Path;

use crate::CliConfig;
use crate::shared::CommandResult;

use super::types::{PluginComponentRef, PluginDetailOutput};

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    // CLI: user-provided partial lookup
    #[arg(help = "Plugin ID (directory name)")]
    pub id: String,
}

pub fn execute(args: &ShowArgs, _config: &CliConfig) -> Result<CommandResult<PluginDetailOutput>> {
    let plugins_path = get_plugins_path()?;
    let plugin_dir = plugins_path.join(&args.id);

    if !plugin_dir.exists() {
        return Err(anyhow!("Plugin '{}' not found", args.id));
    }

    let config_path = plugin_dir.join("config.yaml");
    if !config_path.exists() {
        return Err(anyhow!("Plugin '{}' has no config.yaml file", args.id));
    }

    let plugin_file = parse_plugin_config(&config_path)?;
    let plugin = &plugin_file.plugin;

    let hooks_count = count_hooks(&plugin.hooks);

    let output = PluginDetailOutput {
        id: systemprompt_identifiers::PluginId::new(plugin.id.clone()),
        name: plugin.name.clone(),
        description: plugin.description.clone(),
        version: plugin.version.clone(),
        enabled: plugin.enabled,
        skills: PluginComponentRef {
            source: plugin.skills.source,
            filter: plugin.skills.filter,
            include: plugin.skills.include.clone(),
            exclude: plugin.skills.exclude.clone(),
        },
        agents: PluginComponentRef {
            source: plugin.agents.source,
            filter: plugin.agents.filter,
            include: plugin.agents.include.clone(),
            exclude: plugin.agents.exclude.clone(),
        },
        mcp_servers: plugin.mcp_servers.clone(),
        hooks_count,
        scripts: plugin.scripts.iter().map(|s| s.name.clone()).collect(),
        keywords: plugin.keywords.clone(),
        category: plugin.category.clone(),
        author: plugin.author.name.clone(),
    };

    Ok(CommandResult::card(output).with_title(format!("Plugin: {}", args.id)))
}

fn get_plugins_path() -> Result<std::path::PathBuf> {
    let profile = systemprompt_models::ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.plugins()))
}

fn parse_plugin_config(config_path: &Path) -> Result<systemprompt_models::PluginConfigFile> {
    let content = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let plugin_file: systemprompt_models::PluginConfigFile = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", config_path.display()))?;
    Ok(plugin_file)
}

fn count_hooks(hooks: &systemprompt_models::HookEventsConfig) -> usize {
    systemprompt_models::HookEvent::ALL_VARIANTS
        .iter()
        .map(|event| hooks.matchers_for_event(*event).len())
        .sum()
}
