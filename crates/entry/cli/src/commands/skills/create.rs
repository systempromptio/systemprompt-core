use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;
use std::fs;
use std::path::Path;

use super::types::SkillCreateOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_core_logging::CliService;
use systemprompt_models::ProfileBootstrap;

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long, help = "Skill name/slug (e.g., greeting-agent)")]
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
}

pub async fn execute(
    args: CreateArgs,
    config: &CliConfig,
) -> Result<CommandResult<SkillCreateOutput>> {
    let name = resolve_input(args.name, "name", config, prompt_name)?;
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
            prompt_description().unwrap_or_default()
        } else {
            String::new()
        }
    });

    let instructions = resolve_instructions(
        args.instructions.as_deref(),
        args.instructions_file.as_deref(),
        config,
    )?;

    let tags: Vec<String> = args
        .tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let enabled = args.enabled.unwrap_or(true);

    CliService::info(&format!(
        "Creating skill '{}' (display: {})...",
        name, display_name
    ));

    let skills_path = get_skills_path()?;
    let skill_dir = skills_path.join(&name);

    if skill_dir.exists() {
        return Err(anyhow!(
            "Skill directory already exists: {}. Use 'skills edit' to modify.",
            skill_dir.display()
        ));
    }

    fs::create_dir_all(&skill_dir)
        .with_context(|| format!("Failed to create skill directory: {}", skill_dir.display()))?;

    let index_path = skill_dir.join("index.md");
    let content = build_skill_markdown(&display_name, &description, enabled, &tags, &instructions);

    fs::write(&index_path, content)
        .with_context(|| format!("Failed to write skill file: {}", index_path.display()))?;

    CliService::success(&format!(
        "Skill '{}' created at {}",
        name,
        index_path.display()
    ));

    let output = SkillCreateOutput {
        skill_id: name.replace('-', "_"),
        message: format!(
            "Skill '{}' created successfully at {}",
            name,
            index_path.display()
        ),
        file_path: index_path.to_string_lossy().to_string(),
    };

    Ok(CommandResult::text(output).with_title("Skill Created"))
}

fn get_skills_path() -> Result<std::path::PathBuf> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.skills()))
}

fn validate_skill_name(name: &str) -> Result<()> {
    if name.len() < 3 || name.len() > 50 {
        return Err(anyhow!("Skill name must be between 3 and 50 characters"));
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow!(
            "Skill name must be lowercase alphanumeric with hyphens only"
        ));
    }

    Ok(())
}

fn title_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn resolve_instructions(
    instructions: Option<&str>,
    instructions_file: Option<&str>,
    config: &CliConfig,
) -> Result<String> {
    if let Some(i) = instructions {
        return Ok(i.to_string());
    }

    if let Some(file) = instructions_file {
        let path = Path::new(file);
        return fs::read_to_string(path)
            .with_context(|| format!("Failed to read instructions file: {}", path.display()));
    }

    if config.is_interactive() {
        return prompt_instructions();
    }

    Ok(String::new())
}

fn build_skill_markdown(
    title: &str,
    description: &str,
    enabled: bool,
    tags: &[String],
    instructions: &str,
) -> String {
    let tags_yaml = if tags.is_empty() {
        "[]".to_string()
    } else {
        format!(
            "[{}]",
            tags.iter()
                .map(|t| format!("\"{}\"", t))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    format!(
        r#"---
title: "{}"
description: "{}"
enabled: {}
keywords: {}
---

{}
"#,
        title, description, enabled, tags_yaml, instructions
    )
}

fn prompt_name() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Skill name (slug)")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.len() < 3 {
                return Err("Name must be at least 3 characters");
            }
            if !input
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err("Name must be lowercase alphanumeric with hyphens only");
            }
            Ok(())
        })
        .interact_text()
        .context("Failed to get skill name")
}

fn prompt_display_name(default: &str) -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Display name")
        .default(title_case(default))
        .interact_text()
        .context("Failed to get display name")
}

fn prompt_description() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Description")
        .allow_empty(true)
        .interact_text()
        .context("Failed to get description")
}

fn prompt_instructions() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Instructions (single line, or use --instructions-file)")
        .allow_empty(true)
        .interact_text()
        .context("Failed to get instructions")
}
