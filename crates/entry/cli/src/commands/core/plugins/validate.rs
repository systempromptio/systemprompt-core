use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::path::Path;

use crate::CliConfig;
use crate::shared::CommandResult;

use super::types::{PluginValidateAllOutput, PluginValidateOutput};

#[derive(Debug, Clone, Args)]
pub struct ValidateArgs {
    #[arg(help = "Plugin ID to validate (validates all if omitted)")]
    pub id: Option<String>,
}

pub fn execute(
    args: ValidateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<PluginValidateAllOutput>> {
    let profile = systemprompt_models::ProfileBootstrap::get().context("Failed to get profile")?;
    let plugins_path = std::path::PathBuf::from(profile.paths.plugins());
    let skills_path = std::path::PathBuf::from(profile.paths.skills());

    let plugin_ids = match args.id {
        Some(id) => {
            let plugin_dir = plugins_path.join(&id);
            if !plugin_dir.exists() {
                return Err(anyhow!("Plugin '{}' not found", id));
            }
            vec![id]
        },
        None => collect_plugin_ids(&plugins_path)?,
    };

    let mut results = Vec::new();

    for plugin_id in &plugin_ids {
        let result = validate_plugin(plugin_id, &plugins_path, &skills_path);
        results.push(result);
    }

    let output = PluginValidateAllOutput { results };

    Ok(CommandResult::table(output)
        .with_title("Plugin Validation Results")
        .with_columns(vec![
            "plugin_id".to_string(),
            "valid".to_string(),
            "errors".to_string(),
            "warnings".to_string(),
        ]))
}

fn collect_plugin_ids(plugins_path: &Path) -> Result<Vec<String>> {
    if !plugins_path.exists() {
        return Ok(Vec::new());
    }

    let mut ids = Vec::new();
    for entry in std::fs::read_dir(plugins_path)? {
        let entry = entry?;
        if entry.path().is_dir() && entry.path().join("config.yaml").exists() {
            if let Some(name) = entry.file_name().to_str() {
                ids.push(name.to_string());
            }
        }
    }
    ids.sort();
    Ok(ids)
}

fn validate_plugin(
    plugin_id: &str,
    plugins_path: &Path,
    skills_path: &Path,
) -> PluginValidateOutput {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let config_path = plugins_path.join(plugin_id).join("config.yaml");
    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => {
            errors.push(format!("Failed to read config.yaml: {}", e));
            return PluginValidateOutput {
                plugin_id: systemprompt_identifiers::PluginId::new(plugin_id),
                valid: false,
                errors,
                warnings,
            };
        },
    };

    let plugin_file: systemprompt_models::PluginConfigFile = match serde_yaml::from_str(&content) {
        Ok(p) => p,
        Err(e) => {
            errors.push(format!("Failed to parse config.yaml: {}", e));
            return PluginValidateOutput {
                plugin_id: systemprompt_identifiers::PluginId::new(plugin_id),
                valid: false,
                errors,
                warnings,
            };
        },
    };

    let plugin = &plugin_file.plugin;

    if let Err(e) = plugin.validate(plugin_id) {
        errors.push(format!("{}", e));
    }

    if plugin.id != plugin_id {
        warnings.push(format!(
            "Plugin id '{}' does not match directory name '{}'",
            plugin.id, plugin_id
        ));
    }

    validate_skill_refs(plugin, skills_path, &mut errors, &mut warnings);
    validate_scripts(plugin, plugins_path, plugin_id, &mut errors);

    PluginValidateOutput {
        plugin_id: systemprompt_identifiers::PluginId::new(plugin_id),
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

fn validate_skill_refs(
    plugin: &systemprompt_models::PluginConfig,
    skills_path: &Path,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    if plugin.skills.source == systemprompt_models::ComponentSource::Explicit {
        for skill_id in &plugin.skills.include {
            let skill_dir = skills_path.join(skill_id);
            if !skill_dir.exists() {
                errors.push(format!("Referenced skill '{}' not found", skill_id));
            }
        }
    }

    if !skills_path.exists()
        && plugin.skills.source == systemprompt_models::ComponentSource::Instance
    {
        warnings.push("Skills directory does not exist".to_string());
    }
}

fn validate_scripts(
    plugin: &systemprompt_models::PluginConfig,
    plugins_path: &Path,
    plugin_id: &str,
    errors: &mut Vec<String>,
) {
    for script in &plugin.scripts {
        let script_path = plugins_path.join(plugin_id).join(&script.source);
        if !script_path.exists() {
            errors.push(format!(
                "Script '{}' not found at {}",
                script.name,
                script_path.display()
            ));
        }
    }
}
