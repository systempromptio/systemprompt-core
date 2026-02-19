use anyhow::{Context, Result};
use clap::Args;
use std::path::Path;

use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_models::{HookEvent, HookEventsConfig, HookMatcher, PluginConfigFile};

use super::types::{HookEntry, HookListOutput};

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs;

pub fn execute(_args: ListArgs, _config: &CliConfig) -> Result<CommandResult<HookListOutput>> {
    let profile = systemprompt_models::ProfileBootstrap::get().context("Failed to get profile")?;
    let plugins_path = std::path::PathBuf::from(profile.paths.plugins());

    let hooks = scan_hooks(&plugins_path)?;
    let output = HookListOutput { hooks };

    Ok(CommandResult::table(output)
        .with_title("Hooks")
        .with_columns(vec![
            "plugin_id".to_string(),
            "event".to_string(),
            "matcher".to_string(),
            "hook_type".to_string(),
            "command".to_string(),
        ]))
}

fn scan_hooks(plugins_path: &Path) -> Result<Vec<HookEntry>> {
    if !plugins_path.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    for dir_entry in std::fs::read_dir(plugins_path)? {
        let dir_entry = dir_entry?;
        let path = dir_entry.path();
        if !path.is_dir() {
            continue;
        }

        let config_path = path.join("config.yaml");
        if !config_path.exists() {
            continue;
        }

        let content = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %config_path.display(), error = %e, "Failed to read hook config");
                continue;
            },
        };

        let plugin_file: PluginConfigFile = match serde_yaml::from_str(&content) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(path = %config_path.display(), error = %e, "Failed to parse hook config");
                continue;
            },
        };

        let plugin_id = plugin_file.plugin.id.clone();
        extract_hook_entries(&plugin_id, &plugin_file.plugin.hooks, &mut entries);
    }

    Ok(entries)
}

fn extract_hook_entries(plugin_id: &str, hooks: &HookEventsConfig, entries: &mut Vec<HookEntry>) {
    for event in HookEvent::ALL_VARIANTS {
        extract_event_hooks(plugin_id, *event, hooks.matchers_for_event(*event), entries);
    }
}

fn extract_event_hooks(
    plugin_id: &str,
    event: HookEvent,
    matchers: &[HookMatcher],
    entries: &mut Vec<HookEntry>,
) {
    for matcher in matchers {
        for action in &matcher.hooks {
            let hook_type = format!("{:?}", action.hook_type).to_lowercase();
            entries.push(HookEntry {
                plugin_id: plugin_id.to_string(),
                event: event.as_str().to_string(),
                matcher: matcher.matcher.clone(),
                hook_type,
                command: action.command.clone(),
            });
        }
    }
}
