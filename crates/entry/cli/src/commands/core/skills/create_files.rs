use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_agent::services::skills::SkillIngestionService;
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::SourceId;
use systemprompt_loader::ConfigLoader;

pub fn get_skills_path() -> Result<std::path::PathBuf> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.skills()))
}

pub fn build_skill_markdown(description: &str, instructions: &str) -> String {
    format!(
        "---\ndescription: \"{description}\"\n---\n\n{instructions}\n",
        description = description,
        instructions = instructions
    )
}

pub fn build_skill_config(
    name: &str,
    display_name: &str,
    description: &str,
    enabled: bool,
    tags: &[String],
) -> String {
    let tags_yaml = if tags.is_empty() {
        "[]".to_string()
    } else {
        tags.iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"id: {name}
name: "{display_name}"
description: "{description}"
enabled: {enabled}
version: "1.0.0"
file: "SKILL.md"
assigned_agents:
  - content
tags:
{tags_yaml}"#,
        name = name,
        display_name = display_name,
        description = description,
        enabled = enabled,
        tags_yaml = tags_yaml
    )
}

pub async fn sync_skill_to_db() -> Result<()> {
    let db_url = SecretsBootstrap::database_url()
        .context("Database URL not configured")?
        .to_string();

    let write_url = SecretsBootstrap::database_write_url()
        .ok()
        .flatten()
        .map(str::to_string);

    let database = Database::from_config_with_write("postgres", &db_url, write_url.as_deref())
        .await
        .context("Failed to connect to database")?;

    let db: DbPool = Arc::new(database);
    let ingestion_service = SkillIngestionService::new(&db)?;

    let services_config = ConfigLoader::load().context("Failed to load services config")?;

    ingestion_service
        .ingest_config(&services_config.skills, SourceId::new("cli"), false)
        .await
        .context("Failed to sync skill to database")?;

    Ok(())
}
