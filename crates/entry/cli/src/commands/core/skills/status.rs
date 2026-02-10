use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;

use super::types::{SkillStatusOutput, SkillStatusRow, SkillStatusSummary, SyncStatus};
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_database::{Database, DbPool};
use systemprompt_logging::CliService;
use systemprompt_models::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_sync::diff::SkillsDiffCalculator;

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[arg(help = "Skill ID to show status (optional)")]
    pub name: Option<String>,
}

pub async fn execute(
    args: StatusArgs,
    _config: &CliConfig,
) -> Result<CommandResult<SkillStatusOutput>> {
    CliService::section("Skills Database Status");

    let db = create_db_provider().await?;
    let skills_path = get_skills_path()?;

    let calculator = SkillsDiffCalculator::new(&db)?;
    let diff = calculator
        .calculate_diff(&skills_path)
        .await
        .context("Failed to calculate diff")?;

    let mut skills: Vec<SkillStatusRow> = Vec::new();

    for item in &diff.added {
        if let Some(ref name) = args.name {
            if &item.skill_id != name {
                continue;
            }
        }
        skills.push(SkillStatusRow {
            skill_id: item.skill_id.clone(),
            name: item.name.clone().unwrap_or_else(String::new),
            on_disk: true,
            in_db: false,
            status: SyncStatus::DiskOnly,
        });
    }

    for item in &diff.removed {
        if let Some(ref name) = args.name {
            if &item.skill_id != name {
                continue;
            }
        }
        skills.push(SkillStatusRow {
            skill_id: item.skill_id.clone(),
            name: item.name.clone().unwrap_or_else(String::new),
            on_disk: false,
            in_db: true,
            status: SyncStatus::DbOnly,
        });
    }

    for item in &diff.modified {
        if let Some(ref name) = args.name {
            if &item.skill_id != name {
                continue;
            }
        }
        skills.push(SkillStatusRow {
            skill_id: item.skill_id.clone(),
            name: item.name.clone().unwrap_or_else(String::new),
            on_disk: true,
            in_db: true,
            status: SyncStatus::Modified,
        });
    }

    let synced_count = if args.name.is_some() {
        0
    } else {
        diff.unchanged
    };

    skills.sort_by(|a, b| a.skill_id.cmp(&b.skill_id));

    let summary = SkillStatusSummary {
        total: skills.len() + synced_count,
        synced: synced_count,
        disk_only: diff.added.len(),
        db_only: diff.removed.len(),
        modified: diff.modified.len(),
    };

    display_summary(&summary);

    let output = SkillStatusOutput { skills, summary };

    Ok(CommandResult::table(output)
        .with_title("Skills Sync Status")
        .with_columns(vec![
            "skill_id".to_string(),
            "name".to_string(),
            "on_disk".to_string(),
            "in_db".to_string(),
            "status".to_string(),
        ]))
}

fn get_skills_path() -> Result<std::path::PathBuf> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.skills()))
}

async fn create_db_provider() -> Result<DbPool> {
    let url = SecretsBootstrap::database_url()
        .context("Database URL not configured")?
        .to_string();

    let database = Database::from_config("postgres", &url)
        .await
        .context("Failed to connect to database")?;

    Ok(Arc::new(database))
}

fn display_summary(summary: &SkillStatusSummary) {
    CliService::key_value("Total", &summary.total.to_string());
    CliService::key_value("Synced", &summary.synced.to_string());

    if summary.disk_only > 0 {
        CliService::key_value("Disk Only", &format!("+ {}", summary.disk_only));
    }

    if summary.db_only > 0 {
        CliService::key_value("DB Only", &format!("- {}", summary.db_only));
    }

    if summary.modified > 0 {
        CliService::key_value("Modified", &format!("~ {}", summary.modified));
    }
}
