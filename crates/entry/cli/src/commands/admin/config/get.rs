use anyhow::Result;
use clap::Args;

use super::types::{
    get_yaml_value, parse_config_path, read_yaml_file, yaml_to_json, ConfigGetOutput,
};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct GetArgs {
    #[arg(value_name = "PATH")]
    pub path: String,
}

pub fn execute(args: GetArgs, _config: &CliConfig) -> Result<CommandResult<ConfigGetOutput>> {
    let (section, key) = parse_config_path(&args.path)?;

    let file_path = section.file_path()?;

    if !file_path.exists() {
        anyhow::bail!(
            "Config file not found: {}\nSection '{}' may not be configured.",
            file_path.display(),
            section
        );
    }

    let content = read_yaml_file(&file_path)?;

    let value = get_yaml_value(&content, &key).ok_or_else(|| {
        anyhow::anyhow!(
            "Key '{}' not found in {} configuration",
            if key.is_empty() { "(root)" } else { &key },
            section
        )
    })?;

    let output = ConfigGetOutput {
        path: args.path,
        value: yaml_to_json(&value),
    };

    Ok(CommandResult::card(output).with_title("Config Value"))
}
