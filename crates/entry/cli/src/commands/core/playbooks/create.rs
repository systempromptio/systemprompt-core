use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::types::PlaybookCreateOutput;
use crate::interactive::resolve_required;
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_agent::services::playbooks::PlaybookIngestionService;
use systemprompt_database::Database;
use systemprompt_identifiers::SourceId;
use systemprompt_logging::CliService;
use systemprompt_models::{ProfileBootstrap, SecretsBootstrap};

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long, help = "Playbook name/title")]
    pub name: Option<String>,

    #[arg(long, help = "Category (e.g., cli, build, content)")]
    pub category: Option<String>,

    #[arg(long, help = "Domain within category (e.g., deploy, agents)")]
    pub domain: Option<String>,

    #[arg(long, help = "Description of the playbook")]
    pub description: Option<String>,

    #[arg(long, help = "Playbook instructions")]
    pub instructions: Option<String>,

    #[arg(long, help = "File containing playbook instructions")]
    pub instructions_file: Option<String>,

    #[arg(long, help = "Comma-separated tags/keywords")]
    pub tags: Option<String>,

    #[arg(long, help = "Enable the playbook (default: true)")]
    pub enabled: Option<bool>,

    #[arg(long, help = "Skip syncing to database after creation")]
    pub no_sync: bool,
}

pub async fn execute(
    args: CreateArgs,
    config: &CliConfig,
) -> Result<CommandResult<PlaybookCreateOutput>> {
    let category = resolve_required(args.category, "category", config, prompt_category)?;
    validate_identifier(&category, "category")?;

    let domain = resolve_required(args.domain, "domain", config, prompt_domain)?;
    validate_identifier(&domain, "domain")?;

    let playbook_id = format!("{}_{}", category, domain);

    let name = args.name.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_name(&playbook_id).unwrap_or_else(|_| title_case(&playbook_id))
        } else {
            title_case(&playbook_id)
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
        "Creating playbook '{}' ({}/{})...",
        name, category, domain
    ));

    let playbooks_path = get_playbooks_path()?;
    let category_dir = playbooks_path.join(&category);
    let playbook_file = category_dir.join(format!("{}.md", domain));

    if playbook_file.exists() {
        return Err(anyhow!(
            "Playbook already exists: {}. Use 'playbooks edit' to modify.",
            playbook_file.display()
        ));
    }

    fs::create_dir_all(&category_dir).with_context(|| {
        format!(
            "Failed to create category directory: {}",
            category_dir.display()
        )
    })?;

    let content = build_playbook_markdown(
        &PlaybookFrontmatterParams {
            title: &name,
            slug: &playbook_id,
            description: &description,
            enabled,
            tags: &tags,
        },
        &instructions,
    );

    fs::write(&playbook_file, content)
        .with_context(|| format!("Failed to write playbook file: {}", playbook_file.display()))?;

    CliService::success(&format!(
        "Playbook '{}' created at {}",
        playbook_id,
        playbook_file.display()
    ));

    let mut synced_to_db = false;
    if !args.no_sync {
        match sync_playbook_to_db(&playbook_file).await {
            Ok(()) => {
                CliService::success("Playbook synced to database");
                synced_to_db = true;
            },
            Err(e) => {
                CliService::warning(&format!(
                    "Playbook created but not synced to database: {}. Run 'playbooks sync' \
                     manually.",
                    e
                ));
            },
        }
    }

    let message = if synced_to_db {
        format!(
            "Playbook '{}' created and synced to database at {}",
            playbook_id,
            playbook_file.display()
        )
    } else {
        format!(
            "Playbook '{}' created at {}",
            playbook_id,
            playbook_file.display()
        )
    };

    let output = PlaybookCreateOutput {
        playbook_id,
        message,
        file_path: playbook_file.to_string_lossy().to_string(),
    };

    Ok(CommandResult::text(output).with_title("Playbook Created"))
}

fn get_playbooks_path() -> Result<std::path::PathBuf> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(format!(
        "{}/playbook",
        profile.paths.services
    )))
}

fn validate_identifier(value: &str, field_name: &str) -> Result<()> {
    if value.len() < 2 || value.len() > 30 {
        return Err(anyhow!(
            "{} must be between 2 and 30 characters",
            field_name
        ));
    }

    if !value
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow!(
            "{} must be lowercase alphanumeric with hyphens only",
            field_name
        ));
    }

    Ok(())
}

fn title_case(s: &str) -> String {
    s.split(['_', '-'])
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

struct PlaybookFrontmatterParams<'a> {
    title: &'a str,
    slug: &'a str,
    description: &'a str,
    enabled: bool,
    tags: &'a [String],
}

fn build_playbook_markdown(params: &PlaybookFrontmatterParams<'_>, instructions: &str) -> String {
    let keywords_yaml = if params.tags.is_empty() {
        "keywords: []".to_string()
    } else {
        let tags_list = params
            .tags
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n");
        format!("keywords:\n{}", tags_list)
    };

    format!(
        r#"---
title: "{title}"
slug: "{slug}"
description: "{description}"
enabled: {enabled}
{keywords}
---

{instructions}
"#,
        title = params.title,
        slug = params.slug,
        description = params.description,
        enabled = params.enabled,
        keywords = keywords_yaml,
        instructions = instructions
    )
}

fn prompt_category() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Category (e.g., cli, build, content)")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.len() < 2 {
                return Err("Category must be at least 2 characters");
            }
            if !input
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err("Category must be lowercase alphanumeric with hyphens only");
            }
            Ok(())
        })
        .interact_text()
        .context("Failed to get category")
}

fn prompt_domain() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Domain (e.g., deploy, agents)")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.len() < 2 {
                return Err("Domain must be at least 2 characters");
            }
            if !input
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err("Domain must be lowercase alphanumeric with hyphens only");
            }
            Ok(())
        })
        .interact_text()
        .context("Failed to get domain")
}

fn prompt_name(default: &str) -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Playbook name")
        .default(title_case(default))
        .interact_text()
        .context("Failed to get playbook name")
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

async fn sync_playbook_to_db(playbook_file: &Path) -> Result<()> {
    let db_url = SecretsBootstrap::database_url()
        .context("Database URL not configured")?
        .to_string();

    let database = Database::from_config("postgres", &db_url)
        .await
        .context("Failed to connect to database")?;

    let parent_dir = playbook_file
        .parent()
        .ok_or_else(|| anyhow!("Invalid playbook file path"))?;

    let ingestion_service = PlaybookIngestionService::new(Arc::new(database));

    ingestion_service
        .ingest_directory(parent_dir, SourceId::new("cli"), false)
        .await
        .context("Failed to sync playbook to database")?;

    Ok(())
}
