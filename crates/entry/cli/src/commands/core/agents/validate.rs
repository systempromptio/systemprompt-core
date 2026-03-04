use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_logging::CliService;
use systemprompt_models::AGENT_CONFIG_FILENAME;

use crate::shared::CommandResult;
use crate::CliConfig;

use super::types::{get_agents_path, scan_agent_dirs, validate_agent_config};

#[derive(Debug, Clone, Args)]
pub struct ValidateArgs {
    #[arg(help = "Specific agent ID to validate (optional, validates all if omitted)")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidateOutput {
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub errors: Vec<ValidationEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationEntry {
    pub agent_id: String,
    pub error: String,
}

pub fn execute(args: &ValidateArgs, _config: &CliConfig) -> Result<CommandResult<ValidateOutput>> {
    let agents_path = get_agents_path()?;

    let dirs = if let Some(ref name) = args.name {
        let agent_dir = agents_path.join(name);
        if !agent_dir.exists() {
            anyhow::bail!("Agent '{}' not found", name);
        }
        vec![(name.clone(), agent_dir)]
    } else {
        scan_agent_dirs(&agents_path)?
    };

    let mut valid = 0;
    let mut errors = Vec::new();

    for (dir_name, agent_dir) in &dirs {
        let config_path = agent_dir.join(AGENT_CONFIG_FILENAME);
        match validate_agent_config(&config_path, dir_name) {
            Ok(()) => {
                valid += 1;
                CliService::success(&format!("{dir_name}: valid"));
            },
            Err(e) => {
                let msg = format!("{e:#}");
                CliService::error(&format!("{dir_name}: {msg}"));
                errors.push(ValidationEntry {
                    agent_id: dir_name.clone(),
                    error: msg,
                });
            },
        }
    }

    let total = dirs.len();
    let invalid = errors.len();

    if invalid == 0 {
        CliService::success(&format!("All {total} agent configurations are valid"));
    } else {
        CliService::warning(&format!(
            "{invalid} of {total} agent configurations have errors"
        ));
    }

    let output = ValidateOutput {
        total,
        valid,
        invalid,
        errors,
    };

    Ok(CommandResult::text(output).with_title("Agent Validation"))
}
