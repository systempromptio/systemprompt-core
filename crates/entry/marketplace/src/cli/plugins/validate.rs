//! `plugins validate` subcommand.
//!
//! Checks one or all plugins of the loaded services configuration (which the
//! loader has already parsed from each `config.yaml`) for invalid fields,
//! missing skill references and missing script sources, returning a
//! per-plugin pass/fail report.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::path::Path;

use systemprompt_extension::cli::{CommandHost, CommandOutput};

use super::types::{PluginValidateAllOutput, PluginValidateOutput};
use systemprompt_identifiers::PluginId;
use systemprompt_models::path_confine::{
    PathConfinementError, confine_within, entries_within, locate_within, read_within,
    single_segment,
};

#[derive(Debug, Clone, Args)]
pub struct ValidateArgs {
    #[arg(
        help = "Plugin ID to validate (validates all if omitted)",
        value_parser = systemprompt_extension::cli::parsers::parse_plugin_id
    )]
    pub id: Option<PluginId>,
}

pub(super) async fn execute(
    args: ValidateArgs,
    host: &dyn CommandHost,
) -> Result<(CommandOutput, bool)> {
    let tree = host.services_tree()?;
    let plugins_path = tree.plugins();
    let skills_path = tree.skills();
    let services = crate::cli::services_config(host).await?;

    let plugins = super::selected_plugins(&services, args.id.as_ref())?;
    let results: Vec<PluginValidateOutput> = plugins
        .into_iter()
        .map(|plugin| validate_plugin(plugin, &plugins_path, &skills_path))
        .collect();

    let valid = results.iter().all(|result| result.valid);
    let output = PluginValidateAllOutput { results };

    Ok((
        CommandOutput::table_of(
            vec!["plugin_id", "valid", "errors", "warnings"],
            &output.results,
        )
        .with_title("Plugin Validation Results"),
        valid,
    ))
}

pub fn validate_plugin(
    plugin: &systemprompt_manifest::PluginConfig,
    plugins_path: &Path,
    skills_path: &Path,
) -> PluginValidateOutput {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if let Err(e) = plugin.validate(plugin.id.as_str()) {
        errors.push(format!("{}", e));
    }

    validate_skill_refs(plugin, skills_path, &mut errors, &mut warnings);
    validate_scripts(plugin, plugins_path, &plugin.id, &mut errors);

    PluginValidateOutput {
        plugin_id: plugin.id.clone(),
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

fn validate_skill_refs(
    plugin: &systemprompt_manifest::PluginConfig,
    skills_path: &Path,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    if plugin.skills.source == systemprompt_models::plugin::ComponentSource::Explicit {
        let skills = match declared_skills(skills_path) {
            Ok(skills) => skills,
            Err(e) => {
                errors.push(format!("Skills directory cannot be read: {e}"));
                return;
            },
        };
        for skill_id in &plugin.skills.include {
            match skills.get(skill_id.as_str()) {
                None => errors.push(format!(
                    "Referenced skill '{}' does not exist (no skill declares that id)",
                    skill_id
                )),
                Some(skill) => {
                    if let Some(unknown) = &skill.unknown_host {
                        errors.push(format!("Referenced skill '{skill_id}' lists an {unknown}"));
                    }
                    if !skill.enabled {
                        warnings.push(format!(
                            "Referenced skill '{}' is disabled and will not be delivered",
                            skill_id
                        ));
                    }
                },
            }
        }
    }

    if !skills_path.exists()
        && plugin.skills.source == systemprompt_models::plugin::ComponentSource::Instance
    {
        warnings.push("Skills directory does not exist".to_owned());
    }
}

struct DeclaredSkill {
    enabled: bool,
    unknown_host: Option<systemprompt_models::bridge::host::UnknownHostKind>,
}

fn declared_skills(
    skills_path: &Path,
) -> Result<std::collections::HashMap<String, DeclaredSkill>, PathConfinementError> {
    let mut skills = std::collections::HashMap::new();
    if !skills_path.exists() {
        return Ok(skills);
    }
    for entry in entries_within(skills_path, skills_path)? {
        if !entry.is_dir {
            continue;
        }
        let dir = entry.path;
        let Some(config_path) = locate_within(&dir, &dir.join("config.yaml"))? else {
            continue;
        };
        let content = read_within(&dir, &config_path)?;
        let Ok(config) = serde_yaml::from_str::<systemprompt_manifest::DiskSkillConfig>(&content)
        else {
            continue;
        };
        let id = config.id.as_ref().map_or_else(
            || entry.name.to_string_lossy().into_owned(),
            |id| id.as_str().to_owned(),
        );
        skills.insert(
            id,
            DeclaredSkill {
                enabled: config.enabled,
                unknown_host: config.host_kinds().err(),
            },
        );
    }
    Ok(skills)
}

fn validate_scripts(
    plugin: &systemprompt_manifest::PluginConfig,
    plugins_path: &Path,
    plugin_id: &PluginId,
    errors: &mut Vec<String>,
) {
    let plugin_dir = plugins_path.join(plugin_id.as_str());
    for script in &plugin.scripts {
        if let Err(e) = single_segment("script name", &script.name) {
            errors.push(format!("Script '{}': {}", script.name, e));
        }
        let script_path = plugin_dir.join(&script.source);
        if !script_path.exists() {
            errors.push(format!(
                "Script '{}' not found at {}",
                script.name,
                script_path.display()
            ));
        } else if let Err(e) = confine_within(&plugin_dir, &script_path) {
            errors.push(format!("Script '{}': {}", script.name, e));
        }
    }
}
