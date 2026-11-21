//! `core plugins show` command for one plugin config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use std::path::Path;

use systemprompt_extension::cli::{CommandHost, CommandOutput};

use super::types::{PluginComponentRef, PluginDetailOutput};
use systemprompt_identifiers::PluginId;
use systemprompt_models::path_confine::confine_within;

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "Plugin ID (directory name)", value_parser = systemprompt_extension::cli::parsers::parse_plugin_id)]
    pub id: PluginId,
}

pub(super) fn execute(args: &ShowArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let plugins_path = host.services_tree()?.plugins();
    execute_with_path(args, &plugins_path)
}

pub fn execute_with_path(args: &ShowArgs, plugins_path: &Path) -> Result<CommandOutput> {
    let plugin_dir = plugins_path.join(args.id.as_str());

    if !plugin_dir.exists() {
        return Err(anyhow!("Plugin '{}' not found", args.id));
    }
    let plugin_dir = confine_within(plugins_path, &plugin_dir)?;

    let config_path = plugin_dir.join("config.yaml");
    if !config_path.exists() {
        return Err(anyhow!("Plugin '{}' has no config.yaml file", args.id));
    }
    let config_path = confine_within(&plugin_dir, &config_path)?;

    let plugin_file = super::read_plugin_config(&config_path)?;
    let plugin = &plugin_file.plugin;

    let output = PluginDetailOutput {
        id: PluginId::new(plugin.id.clone()),
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
        scripts: plugin.scripts.iter().map(|s| s.name.clone()).collect(),
        keywords: plugin.keywords.clone(),
        category: plugin.category.clone(),
        author: plugin.author.name.clone(),
    };

    Ok(CommandOutput::card_value(
        format!("Plugin: {}", args.id),
        &output,
    ))
}
