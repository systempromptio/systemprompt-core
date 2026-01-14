use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_extension::ExtensionRegistry;

use super::types::ExtensionConfigOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ConfigArgs {
    #[arg(help = "Extension ID")]
    pub id: String,
}

pub fn execute(args: ConfigArgs, _config: &CliConfig) -> Result<CommandResult<ExtensionConfigOutput>> {
    let registry = ExtensionRegistry::discover();

    let ext = registry
        .get(&args.id)
        .ok_or_else(|| anyhow!("Extension '{}' not found", args.id))?;

    let output = ExtensionConfigOutput {
        extension_id: ext.id().to_string(),
        config_prefix: ext.config_prefix().map(String::from),
        config_schema: ext.config_schema(),
        has_config: ext.has_config(),
    };

    Ok(CommandResult::card(output).with_title(format!("Extension Config: {}", args.id)))
}
