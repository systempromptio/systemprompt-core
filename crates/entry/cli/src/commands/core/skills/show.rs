use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::path::Path;
use systemprompt_models::SKILL_CONFIG_FILENAME;

use crate::CliConfig;
use crate::shared::CommandResult;

use super::types::{SkillDetailOutput, parse_skill_from_config};

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "Skill ID (directory name)")]
    pub name: String,
}

pub fn execute(args: &ShowArgs, _config: &CliConfig) -> Result<CommandResult<SkillDetailOutput>> {
    let skills_path = get_skills_path()?;
    show_skill_detail(&args.name, &skills_path)
}

fn get_skills_path() -> Result<std::path::PathBuf> {
    let profile = systemprompt_models::ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.skills()))
}

fn show_skill_detail(
    skill_id: &str,
    skills_path: &Path,
) -> Result<CommandResult<SkillDetailOutput>> {
    let skill_dir = skills_path.join(skill_id);

    if !skill_dir.exists() {
        return Err(anyhow!("Skill '{}' not found", skill_id));
    }

    let config_path = skill_dir.join(SKILL_CONFIG_FILENAME);

    if !config_path.exists() {
        return Err(anyhow!(
            "Skill '{}' has no {} file",
            skill_id,
            SKILL_CONFIG_FILENAME
        ));
    }

    let parsed = parse_skill_from_config(&config_path, &skill_dir)?;

    let instructions_preview = parsed.instructions.chars().take(200).collect::<String>()
        + if parsed.instructions.len() > 200 {
            "..."
        } else {
            ""
        };

    let output = SkillDetailOutput {
        skill_id: skill_id.to_string(),
        name: parsed.name,
        description: parsed.description,
        enabled: parsed.enabled,
        tags: parsed.tags,
        category: parsed.category,
        file_path: config_path.to_string_lossy().to_string(),
        instructions_preview,
    };

    Ok(CommandResult::card(output).with_title(format!("Skill: {}", skill_id)))
}
