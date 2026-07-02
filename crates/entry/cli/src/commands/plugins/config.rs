use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_extension::ExtensionRegistry;

use super::types::{ExtensionConfigListOutput, ExtensionConfigOutput, ExtensionConfigSummary};
use crate::CliConfig;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct ConfigArgs {
    #[arg(help = "Extension ID (optional - lists all if not specified)")]
    pub id: Option<String>,
}

pub fn execute(args: &ConfigArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let registry = ExtensionRegistry::discover()?;

    if let Some(id) = &args.id {
        let ext = registry
            .get(id)
            .ok_or_else(|| anyhow!("Extension '{}' not found", id))?;

        let output = ExtensionConfigOutput {
            extension_id: systemprompt_identifiers::PluginId::new(ext.id()),
            config_prefix: ext.config_prefix().map(String::from),
            config_schema: ext.config_schema(),
            has_config: ext.has_config(),
        };

        Ok(CommandOutput::card_value(
            format!("Extension Config: {}", id),
            &output,
        ))
    } else {
        let mut extensions: Vec<ExtensionConfigSummary> = registry
            .extensions()
            .iter()
            .map(|ext| ExtensionConfigSummary {
                extension_id: systemprompt_identifiers::PluginId::new(ext.id()),
                config_prefix: ext.config_prefix().map(String::from),
                has_config: ext.has_config(),
            })
            .collect();

        extensions.sort_by(|a, b| a.extension_id.cmp(&b.extension_id));
        let total = extensions.len();

        let output = ExtensionConfigListOutput { extensions, total };

        Ok(CommandOutput::table_of(
            vec!["extension_id", "config_prefix", "has_config"],
            &output.extensions,
        )
        .with_title("Extension Configurations"))
    }
}
