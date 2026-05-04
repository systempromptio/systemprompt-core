use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::fs;

use super::create_files::{
    build_skill_config, build_skill_markdown, get_skills_path, sync_skill_to_db,
};
use super::create_prompts::{
    check_normalized_conflicts, prompt_description, prompt_display_name, prompt_name,
    resolve_instructions, title_case, validate_skill_name,
};
use super::types::SkillCreateOutput;
use crate::CliConfig;
use crate::interactive::resolve_required;
use crate::shared::CommandResult;
use systemprompt_identifiers::SkillId;
use systemprompt_logging::CliService;

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long, help = "Skill name/slug (e.g., greeting_agent)")]
    pub name: Option<String>,

    #[arg(long, help = "Display name for the skill")]
    pub display_name: Option<String>,

    #[arg(long, help = "Description of the skill")]
    pub description: Option<String>,

    #[arg(long, help = "Skill instructions")]
    pub instructions: Option<String>,

    #[arg(long, help = "File containing skill instructions")]
    pub instructions_file: Option<String>,

    #[arg(long, help = "Comma-separated tags")]
    pub tags: Option<String>,

    #[arg(long, help = "Enable the skill (default: true)")]
    pub enabled: Option<bool>,

    #[arg(long, help = "Skip syncing to database after creation")]
    pub no_sync: bool,
}

pub async fn execute(
    args: CreateArgs,
    config: &CliConfig,
) -> Result<CommandResult<SkillCreateOutput>> {
    let name = resolve_required(args.name, "name", config, prompt_name)?;
    validate_skill_name(&name)?;

    let display_name = args.display_name.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_display_name(&name).unwrap_or_else(|_| title_case(&name))
        } else {
            title_case(&name)
        }
    });

    let description = args.description.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_description().unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to prompt for description");
                String::new()
            })
        } else {
            String::new()
        }
    });

    let instructions = resolve_instructions(
        args.instructions.as_deref(),
        args.instructions_file.as_deref(),
        config,
    )?;

    let tags: Vec<String> = args.tags.map_or_else(Vec::new, |t| {
        t.split(',').map(|s| s.trim().to_string()).collect()
    });

    let enabled = args.enabled.unwrap_or(true);

    CliService::info(&format!(
        "Creating skill '{}' (display: {})...",
        name, display_name
    ));

    let skills_path = get_skills_path()?;
    check_normalized_conflicts(&name, &skills_path)?;

    let skill_dir = skills_path.join(&name);

    if skill_dir.exists() {
        return Err(anyhow!(
            "Skill directory already exists: {}. Use 'skills edit' to modify.",
            skill_dir.display()
        ));
    }

    fs::create_dir_all(&skill_dir)
        .with_context(|| format!("Failed to create skill directory: {}", skill_dir.display()))?;

    let skill_path = skill_dir.join("SKILL.md");
    let content = build_skill_markdown(&description, &instructions);

    fs::write(&skill_path, &content)
        .with_context(|| format!("Failed to write skill file: {}", skill_path.display()))?;

    let config_content = build_skill_config(&name, &display_name, &description, enabled, &tags);
    let config_path = skill_dir.join("config.yaml");
    fs::write(&config_path, config_content)
        .with_context(|| format!("Failed to write config: {}", config_path.display()))?;

    CliService::success(&format!(
        "Skill '{}' created at {}",
        name,
        skill_path.display()
    ));

    let mut synced_to_db = false;
    if !args.no_sync {
        match sync_skill_to_db().await {
            Ok(()) => {
                CliService::success("Skill synced to database");
                synced_to_db = true;
            },
            Err(e) => {
                CliService::warning(&format!(
                    "Skill created but not synced to database: {}. Run 'skills sync' manually.",
                    e
                ));
            },
        }
    }

    let message = if synced_to_db {
        format!(
            "Skill '{}' created and synced to database at {}",
            name,
            skill_path.display()
        )
    } else {
        format!("Skill '{}' created at {}", name, skill_path.display())
    };

    let output = SkillCreateOutput {
        skill_id: SkillId::new(name.clone()),
        message,
        file_path: skill_path.to_string_lossy().to_string(),
    };

    Ok(CommandResult::text(output).with_title("Skill Created"))
}
