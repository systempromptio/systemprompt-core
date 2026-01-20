use anyhow::Result;
use clap::Args;

use super::types::{
    parse_config_path, parse_value_string, read_yaml_file, set_yaml_value, write_yaml_file,
    yaml_to_json, yaml_to_json_opt, ConfigSetOutput,
};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct SetArgs {
    #[arg(value_name = "PATH")]
    pub path: String,

    #[arg(value_name = "VALUE")]
    pub value: String,
}

pub fn execute(args: SetArgs, _config: &CliConfig) -> Result<CommandResult<ConfigSetOutput>> {
    let (section, key) = parse_config_path(&args.path)?;

    if key.is_empty() {
        anyhow::bail!(
            "Cannot set entire section. Specify a key path, e.g., '{}.some_key'",
            section
        );
    }

    let file_path = section.file_path()?;

    if !file_path.exists() {
        anyhow::bail!(
            "Config file not found: {}\nSection '{}' may not be configured.",
            file_path.display(),
            section
        );
    }

    let mut content = read_yaml_file(&file_path)?;
    let new_value = parse_value_string(&args.value);

    let old_value = set_yaml_value(&mut content, &key, new_value.clone())?;

    write_yaml_file(&file_path, &content)?;

    let output = ConfigSetOutput {
        path: args.path,
        old_value: yaml_to_json_opt(old_value.as_ref()),
        new_value: yaml_to_json(&new_value),
        file_path: file_path.display().to_string(),
    };

    Ok(CommandResult::card(output).with_title("Config Updated"))
}
