use anyhow::{Context, Result};
use clap::Args;
use std::path::Path;

use crate::CliConfig;
use crate::shared::CommandResult;
use systemprompt_models::{DiskHookConfig, HOOK_CONFIG_FILENAME};

use super::types::{HookEntry, HookListOutput};

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs;

pub(super) fn execute(
    _args: ListArgs,
    _config: &CliConfig,
) -> Result<CommandResult<HookListOutput>> {
    let profile = systemprompt_config::ProfileBootstrap::get().context("Failed to get profile")?;
    let hooks_path = std::path::PathBuf::from(profile.paths.hooks());

    let hooks = scan_hooks(&hooks_path)?;
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

fn scan_hooks(hooks_path: &Path) -> Result<Vec<HookEntry>> {
    if !hooks_path.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    for dir_entry in std::fs::read_dir(hooks_path)? {
        let dir_entry = dir_entry?;
        let path = dir_entry.path();
        if !path.is_dir() {
            continue;
        }

        let config_path = path.join(HOOK_CONFIG_FILENAME);
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

        let config: DiskHookConfig = match serde_yaml::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %config_path.display(), error = %e, "Failed to parse hook config");
                continue;
            },
        };

        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let id_str = if config.id.as_str().is_empty() {
            dir_name
        } else {
            config.id.as_str().to_string()
        };

        entries.push(HookEntry {
            plugin_id: id_str,
            event: config.event.as_str().to_string(),
            matcher: config.matcher.clone(),
            hook_type: "command".to_string(),
            command: if config.command.is_empty() {
                None
            } else {
                Some(config.command.clone())
            },
        });
    }

    Ok(entries)
}
