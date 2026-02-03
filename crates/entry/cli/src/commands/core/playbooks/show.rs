use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::path::Path;

use super::path_helpers::playbook_id_to_path;
use super::types::PlaybookContentOutput;
use crate::shared::CommandResult;

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "Playbook ID (e.g., 'cli_deploy' or 'info_start')")]
    pub playbook_id: String,

    #[arg(long, help = "Output raw markdown without formatting")]
    pub raw: bool,
}

pub fn execute(args: &ShowArgs) -> Result<CommandResult<PlaybookContentOutput>> {
    let playbooks_path = get_playbooks_path()?;

    let md_path = playbook_id_to_path(&playbooks_path, &args.playbook_id)?;

    if !md_path.exists() {
        return Err(anyhow!(
            "Playbook '{}' not found at {}",
            args.playbook_id,
            md_path.display()
        ));
    }

    let parsed = parse_playbook_markdown(&md_path)?;

    let output = PlaybookContentOutput {
        playbook_id: args.playbook_id.clone(),
        name: parsed.name,
        description: parsed.description,
        content: parsed.instructions,
        file_path: md_path.to_string_lossy().to_string(),
    };

    if args.raw {
        Ok(CommandResult::copy_paste(output).with_title(format!("Playbook: {}", args.playbook_id)))
    } else {
        Ok(CommandResult::card(output).with_title(format!("Playbook: {}", args.playbook_id)))
    }
}

fn get_playbooks_path() -> Result<std::path::PathBuf> {
    let profile = systemprompt_models::ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(format!(
        "{}/playbook",
        profile.paths.services
    )))
}

struct ParsedPlaybook {
    name: String,
    description: String,
    instructions: String,
}

fn parse_playbook_markdown(md_path: &Path) -> Result<ParsedPlaybook> {
    let content = std::fs::read_to_string(md_path)
        .with_context(|| format!("Failed to read {}", md_path.display()))?;

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(anyhow!(
            "Invalid frontmatter format in {}",
            md_path.display()
        ));
    }

    let frontmatter: serde_yaml::Value = serde_yaml::from_str(parts[1])
        .with_context(|| format!("Invalid YAML in {}", md_path.display()))?;

    let name = frontmatter
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing title in {}", md_path.display()))?
        .to_string();

    let description = frontmatter
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(ParsedPlaybook {
        name,
        description,
        instructions: parts[2].trim().to_string(),
    })
}
