//! `core plugins list` command scanning plugin configs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::path::Path;
use systemprompt_models::path_confine::{entries_within, locate_within};

use systemprompt_extension::cli::{CommandHost, CommandOutput};

use super::types::{PluginListOutput, PluginSummary};

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs {
    #[arg(long, help = "Show only enabled plugins")]
    pub enabled: bool,

    #[arg(long, help = "Show only disabled plugins", conflicts_with = "enabled")]
    pub disabled: bool,
}

pub(super) fn execute(args: ListArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let plugins_path = host.services_tree()?.plugins();
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

    Ok(CommandOutput::table_of(
        vec![
            "id",
            "name",
            "display_name",
            "enabled",
            "skill_count",
            "agent_count",
        ],
        &output.plugins,
    )
    .with_title("Plugins"))
}

pub fn scan_plugins(plugins_path: &Path) -> Result<Vec<PluginSummary>> {
    if !plugins_path.exists() {
        return Ok(Vec::new());
    }

    let mut plugins = Vec::new();

    for entry in entries_within(plugins_path, plugins_path)? {
        if !entry.is_dir {
            continue;
        }
        let Some(config_path) = locate_within(&entry.path, &entry.path.join("config.yaml"))?
        else {
            continue;
        };

        match super::read_plugin_config(&config_path) {
            Ok(plugin_file) => {
                let plugin = &plugin_file.plugin;
                let mut summary: PluginSummary = plugin.into();
                summary.skill_count = estimate_component_count(&plugin.skills);
                summary.agent_count = estimate_component_count(&plugin.agents);
                plugins.push(summary);
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

fn estimate_component_count(component: &systemprompt_models::plugin::PluginComponentRef) -> usize {
    if component.source == systemprompt_models::plugin::ComponentSource::Explicit {
        component.include.len()
    } else {
        0
    }
}
