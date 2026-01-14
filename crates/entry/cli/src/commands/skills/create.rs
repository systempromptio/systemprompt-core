use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::types::SkillCreateOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_core_agent::services::skills::SkillIngestionService;
use systemprompt_core_database::Database;
use systemprompt_core_logging::CliService;
use systemprompt_identifiers::SourceId;
use systemprompt_models::{ProfileBootstrap, SecretsBootstrap};

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

    #[arg(long, help = "Skill author (default: systemprompt)")]
    pub author: Option<String>,

    #[arg(long, help = "Category for the skill (default: skills)")]
    pub category: Option<String>,

    #[arg(long, help = "Skill type (default: skill)")]
    pub skill_type: Option<String>,

    #[arg(long, help = "Skip syncing to database after creation")]
    pub no_sync: bool,
}

pub async fn execute(args: CreateArgs, config: &CliConfig) -> Result<CommandResult<SkillCreateOutput>> {
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
    let author = args.author.unwrap_or_else(|| "systemprompt".to_string());
    let category = args.category.unwrap_or_else(|| "skills".to_string());
    let skill_type = args.skill_type.unwrap_or_else(|| "skill".to_string());

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

    let index_path = skill_dir.join("index.md");
    let frontmatter_params = SkillFrontmatterParams {
        title: &display_name,
        slug: &name,
        description: &description,
        author: &author,
        category: &category,
        skill_type: &skill_type,
        enabled,
        tags: &tags,
    };
    let content = build_skill_markdown(&frontmatter_params, &instructions);

    fs::write(&index_path, content)
        .with_context(|| format!("Failed to write skill file: {}", index_path.display()))?;

    CliService::success(&format!(
        "Skill '{}' created at {}",
        name,
        index_path.display()
    ));

    let mut synced_to_db = false;
    if !args.no_sync {
        match sync_skill_to_db(&skill_dir).await {
            Ok(()) => {
                CliService::success("Skill synced to database");
                synced_to_db = true;
            }
            Err(e) => {
                CliService::warning(&format!(
                    "Skill created but not synced to database: {}. Run 'skills sync' manually.",
                    e
                ));
            }
        }
    }

    let message = if synced_to_db {
        format!(
            "Skill '{}' created and synced to database at {}",
            name,
            index_path.display()
        )
    } else {
        format!(
            "Skill '{}' created at {}",
            name,
            index_path.display()
        )
    };

    let output = SkillCreateOutput {
        skill_id: name.clone(),
        message,
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
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(anyhow!(
            "Skill name must be lowercase alphanumeric with underscores only"
        ));
    }

    Ok(())
}

fn normalize_skill_name(name: &str) -> String {
    name.replace('-', "_").to_lowercase()
}

fn check_normalized_conflicts(name: &str, skills_dir: &Path) -> Result<()> {
    let normalized_name = normalize_skill_name(name);

    if !skills_dir.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(skills_dir)
        .with_context(|| format!("Failed to read skills directory: {}", skills_dir.display()))?;

    for entry in entries.filter_map(|e| e.ok()) {
        if !entry.path().is_dir() {
            continue;
        }

        let existing_name = entry.file_name().to_string_lossy().to_string();
        let existing_normalized = normalize_skill_name(&existing_name);

        if existing_name == name {
            continue;
        }

        if existing_normalized == normalized_name {
            return Err(anyhow!(
                "Skill '{}' conflicts with existing skill '{}' (same normalized name: '{}'). \
                 Use consistent naming to avoid confusion.",
                name,
                existing_name,
                normalized_name
            ));
        }
    }

    Ok(())
}

fn title_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
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

struct SkillFrontmatterParams<'a> {
    title: &'a str,
    slug: &'a str,
    description: &'a str,
    author: &'a str,
    category: &'a str,
    skill_type: &'a str,
    enabled: bool,
    tags: &'a [String],
}

fn build_skill_markdown(params: &SkillFrontmatterParams<'_>, instructions: &str) -> String {
    let keywords = if params.tags.is_empty() {
        String::new()
    } else {
        params.tags.join(", ")
    };

    let today = Utc::now().format("%Y-%m-%d").to_string();

    format!(
        r#"---
title: "{title}"
slug: "{slug}"
description: "{description}"
author: "{author}"
published_at: "{published_at}"
type: "{skill_type}"
category: "{category}"
keywords: "{keywords}"
enabled: {enabled}
---

{instructions}
"#,
        title = params.title,
        slug = params.slug,
        description = params.description,
        author = params.author,
        published_at = today,
        skill_type = params.skill_type,
        category = params.category,
        keywords = keywords,
        enabled = params.enabled,
        instructions = instructions
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
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return Err("Name must be lowercase alphanumeric with underscores only");
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

async fn sync_skill_to_db(skill_dir: &Path) -> Result<()> {
    let db_url = SecretsBootstrap::database_url()
        .context("Database URL not configured")?
        .to_string();

    let database = Database::from_config("postgres", &db_url)
        .await
        .context("Failed to connect to database")?;

    let ingestion_service = SkillIngestionService::new(Arc::new(database));

    ingestion_service
        .ingest_directory(skill_dir, SourceId::new("cli"), false)
        .await
        .context("Failed to sync skill to database")?;

    Ok(())
}
