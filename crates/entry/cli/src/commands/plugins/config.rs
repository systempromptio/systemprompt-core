use anyhow::{anyhow, Result};
use clap::Args;
use serde::Serialize;
use systemprompt_extension::ExtensionRegistry;

use super::types::{ExtensionConfigListOutput, ExtensionConfigOutput, ExtensionConfigSummary};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ConfigArgs {
    #[arg(help = "Extension ID (optional - lists all if not specified)")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ConfigResult {
    Single(ExtensionConfigOutput),
    List(ExtensionConfigListOutput),
}

pub fn execute(args: &ConfigArgs, _config: &CliConfig) -> Result<CommandResult<ConfigResult>> {
    let registry = ExtensionRegistry::discover();

    match &args.id {
        Some(id) => {
            let ext = registry
                .get(id)
                .ok_or_else(|| anyhow!("Extension '{}' not found", id))?;

            let output = ExtensionConfigOutput {
                extension_id: ext.id().to_string(),
                config_prefix: ext.config_prefix().map(String::from),
                config_schema: ext.config_schema(),
                has_config: ext.has_config(),
            };

            Ok(CommandResult::card(ConfigResult::Single(output))
                .with_title(format!("Extension Config: {}", id)))
        },
        None => {
            let mut extensions: Vec<ExtensionConfigSummary> = registry
                .extensions()
                .iter()
                .map(|ext| ExtensionConfigSummary {
                    extension_id: ext.id().to_string(),
                    config_prefix: ext.config_prefix().map(String::from),
                    has_config: ext.has_config(),
                })
                .collect();

            extensions.sort_by(|a, b| a.extension_id.cmp(&b.extension_id));
            let total = extensions.len();

            let output = ExtensionConfigListOutput { extensions, total };

            Ok(CommandResult::table(ConfigResult::List(output))
                .with_title("Extension Configurations")
                .with_columns(vec![
                    "extension_id".to_string(),
                    "config_prefix".to_string(),
                    "has_config".to_string(),
                ]))
        },
    }
}
