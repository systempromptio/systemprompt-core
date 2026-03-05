use anyhow::{Context, Result};
use clap::Args;
use std::path::Path;

use crate::CliConfig;
use crate::shared::CommandResult;
use systemprompt_models::{HookEvent, PluginConfigFile};

use super::types::{HookValidateEntry, HookValidateOutput};

const PLUGIN_ROOT_VAR: &str = "${CLAUDE_PLUGIN_ROOT}";

#[derive(Debug, Clone, Copy, Args)]
pub struct ValidateArgs;

pub fn execute(
    _args: ValidateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<HookValidateOutput>> {
    let profile = systemprompt_models::ProfileBootstrap::get().context("Failed to get profile")?;
    let plugins_path = std::path::PathBuf::from(profile.paths.plugins());

    let results = validate_all_hooks(&plugins_path)?;
    let output = HookValidateOutput { results };

    Ok(CommandResult::table(output)
        .with_title("Hook Validation Results")
        .with_columns(vec![
            "plugin_id".to_string(),
            "valid".to_string(),
            "errors".to_string(),
        ]))
}

fn validate_all_hooks(plugins_path: &Path) -> Result<Vec<HookValidateEntry>> {
    if !plugins_path.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();

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

        let Ok(content) = std::fs::read_to_string(&config_path) else {
            continue;
        };

        let Ok(plugin_file): Result<PluginConfigFile, _> = serde_yaml::from_str(&content) else {
            results.push(HookValidateEntry {
                plugin_id: dir_entry.file_name().to_string_lossy().to_string(),
                valid: false,
                errors: vec!["Failed to parse config.yaml".to_string()],
            });
            continue;
        };

        let plugin = &plugin_file.plugin;
        let mut errors = Vec::new();

        if let Err(e) = plugin.hooks.validate() {
            errors.push(format!("{}", e));
        }

        validate_hook_scripts(plugin, plugins_path, &mut errors);

        results.push(HookValidateEntry {
            plugin_id: plugin.id.clone(),
            valid: errors.is_empty(),
            errors,
        });
    }

    Ok(results)
}

fn validate_hook_scripts(
    plugin: &systemprompt_models::PluginConfig,
    plugins_path: &Path,
    errors: &mut Vec<String>,
) {
    let all_commands = collect_hook_commands(&plugin.hooks);
    let plugin_dir = plugins_path.join(&plugin.id);

    for cmd in all_commands {
        if cmd.contains(PLUGIN_ROOT_VAR) {
            let relative = cmd.replace(&format!("{}/", PLUGIN_ROOT_VAR), "");
            let script_path = plugin_dir.join(&relative);
            if !script_path.exists() {
                errors.push(format!(
                    "Hook command references missing script: {}",
                    relative
                ));
            }
        }
    }
}

fn collect_hook_commands(hooks: &systemprompt_models::HookEventsConfig) -> Vec<String> {
    let mut commands = Vec::new();
    for event in HookEvent::ALL_VARIANTS {
        for matcher in hooks.matchers_for_event(*event) {
            for action in &matcher.hooks {
                if let Some(cmd) = &action.command {
                    commands.push(cmd.clone());
                }
            }
        }
    }
    commands
}
