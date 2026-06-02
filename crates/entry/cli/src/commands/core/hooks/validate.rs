use anyhow::{Context, Result};
use clap::Args;
use std::path::Path;

use crate::CliConfig;
use crate::shared::CommandOutput;
use systemprompt_models::{DiskHookConfig, HOOK_CONFIG_FILENAME};

use super::types::{HookValidateEntry, HookValidateOutput};

const PLUGIN_ROOT_VAR: &str = "${CLAUDE_PLUGIN_ROOT}";

#[derive(Debug, Clone, Copy, Args)]
pub struct ValidateArgs;

pub(super) fn execute(_args: ValidateArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let profile = systemprompt_config::ProfileBootstrap::get().context("Failed to get profile")?;
    let hooks_path = std::path::PathBuf::from(profile.paths.hooks());

    let results = validate_all_hooks(&hooks_path)?;
    let output = HookValidateOutput { results };

    Ok(
        CommandOutput::table_of(vec!["plugin_id", "valid", "errors"], &output.results)
            .with_title("Hook Validation Results"),
    )
}

fn validate_all_hooks(hooks_path: &Path) -> Result<Vec<HookValidateEntry>> {
    if !hooks_path.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();

    for dir_entry in std::fs::read_dir(hooks_path)? {
        let dir_entry = dir_entry?;
        let path = dir_entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_owned();
        let config_path = path.join(HOOK_CONFIG_FILENAME);
        if !config_path.exists() {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(&config_path) else {
            continue;
        };

        let config: DiskHookConfig = match serde_yaml::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                results.push(HookValidateEntry {
                    plugin_id: dir_name,
                    valid: false,
                    errors: vec![format!("Failed to parse {HOOK_CONFIG_FILENAME}: {e}")],
                });
                continue;
            },
        };

        let mut errors = Vec::new();
        let id_str = if config.id.as_str().is_empty() {
            dir_name.clone()
        } else {
            config.id.as_str().to_owned()
        };

        if config.command.is_empty() {
            errors.push("command must not be empty".to_owned());
        } else {
            validate_hook_command(&config.command, &path, &mut errors);
        }

        results.push(HookValidateEntry {
            plugin_id: id_str,
            valid: errors.is_empty(),
            errors,
        });
    }

    Ok(results)
}

fn validate_hook_command(command: &str, hook_dir: &Path, errors: &mut Vec<String>) {
    if command.contains(PLUGIN_ROOT_VAR) {
        let relative = command.replace(&format!("{PLUGIN_ROOT_VAR}/"), "");
        let script_path = hook_dir.join(&relative);
        if !script_path.exists() {
            errors.push(format!(
                "Hook command references missing script: {relative}"
            ));
        }
    }
}
