use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::fs;
use std::path::Path;

use super::types::SkillDeleteOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_logging::CliService;
use systemprompt_models::ProfileBootstrap;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Skill name to delete")]
    pub name: Option<String>,

    #[arg(long, help = "Delete all skills")]
    pub all: bool,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub fn execute(args: DeleteArgs, config: &CliConfig) -> Result<CommandResult<SkillDeleteOutput>> {
    let skills_path = get_skills_path()?;

    let skills_to_delete: Vec<String> = if args.all {
        list_all_skills(&skills_path)?
    } else {
        let name = resolve_input(args.name, "name", config, || {
            prompt_skill_selection(&skills_path)
        })?;

        let skill_dir = find_skill_dir(&skills_path, &name);
        if skill_dir.is_none() {
            return Err(anyhow!("Skill '{}' not found", name));
        }

        vec![name]
    };

    if skills_to_delete.is_empty() {
        return Ok(CommandResult::text(SkillDeleteOutput {
            deleted: vec![],
            message: "No skills to delete".to_string(),
        })
        .with_title("Delete Skill"));
    }

    if !args.yes {
        if !config.is_interactive() {
            return Err(anyhow!(
                "--yes is required to delete skills in non-interactive mode"
            ));
        }

        let confirm_message = if args.all {
            format!("Delete ALL {} skills?", skills_to_delete.len())
        } else {
            format!("Delete skill '{}'?", skills_to_delete[0])
        };

        if !CliService::confirm(&confirm_message)? {
            CliService::info("Cancelled");
            return Ok(CommandResult::text(SkillDeleteOutput {
                deleted: vec![],
                message: "Operation cancelled".to_string(),
            })
            .with_title("Delete Cancelled"));
        }
    }

    let mut deleted = Vec::new();

    for skill_name in &skills_to_delete {
        CliService::info(&format!("Deleting skill '{}'...", skill_name));

        match delete_skill(&skills_path, skill_name) {
            Ok(()) => {
                CliService::success(&format!("Skill '{}' deleted", skill_name));
                deleted.push(skill_name.clone());
            },
            Err(e) => {
                CliService::error(&format!("Failed to delete skill '{}': {}", skill_name, e));
            },
        }
    }

    let message = match deleted.len() {
        0 => "No skills were deleted".to_string(),
        1 => format!("Skill '{}' deleted successfully", deleted[0]),
        n => format!("{} skill(s) deleted successfully", n),
    };

    let output = SkillDeleteOutput { deleted, message };
    Ok(CommandResult::text(output).with_title("Delete Skill"))
}

fn get_skills_path() -> Result<std::path::PathBuf> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.skills()))
}

fn find_skill_dir(skills_path: &Path, name: &str) -> Option<std::path::PathBuf> {
    let direct = skills_path.join(name);
    if direct.exists() && direct.is_dir() {
        return Some(direct);
    }
    None
}

fn delete_skill(skills_path: &Path, name: &str) -> Result<()> {
    let skill_dir =
        find_skill_dir(skills_path, name).ok_or_else(|| anyhow!("Skill '{}' not found", name))?;

    fs::remove_dir_all(&skill_dir)
        .with_context(|| format!("Failed to remove skill directory: {}", skill_dir.display()))
}

fn list_all_skills(skills_path: &Path) -> Result<Vec<String>> {
    if !skills_path.exists() {
        return Ok(Vec::new());
    }

    let mut skills = Vec::new();

    for entry in fs::read_dir(skills_path)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let has_skill_file = path.join("index.md").exists() || path.join("SKILL.md").exists();

        if has_skill_file {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                skills.push(name.to_string());
            }
        }
    }

    skills.sort();
    Ok(skills)
}

fn prompt_skill_selection(skills_path: &Path) -> Result<String> {
    let skills = list_all_skills(skills_path)?;

    if skills.is_empty() {
        return Err(anyhow!("No skills found"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select skill to delete")
        .items(&skills)
        .default(0)
        .interact()
        .context("Failed to get skill selection")?;

    Ok(skills[selection].clone())
}
