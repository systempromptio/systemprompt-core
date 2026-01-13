use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Select};
use std::sync::Arc;

use super::types::SkillSyncOutput;
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_core_database::{Database, DatabaseProvider};
use systemprompt_core_logging::CliService;
use systemprompt_models::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_sync::{LocalSyncDirection, SkillsDiffResult, SkillsLocalSync};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SyncDirection {
    ToDb,
    ToDisk,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct SyncArgs {
    #[arg(long, value_enum, help = "Sync direction")]
    pub direction: Option<SyncDirection>,

    #[arg(long, help = "Show what would happen without making changes")]
    pub dry_run: bool,

    #[arg(long, help = "Delete items that only exist in target")]
    pub delete_orphans: bool,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub async fn execute(args: SyncArgs, config: &CliConfig) -> Result<CommandResult<SkillSyncOutput>> {
    CliService::section("Skills Sync");

    let spinner = CliService::spinner("Connecting to database...");
    let db = create_db_provider().await?;
    spinner.finish_and_clear();

    let skills_path = get_skills_path()?;

    if !skills_path.exists() {
        anyhow::bail!(
            "Skills path does not exist: {}\nEnsure the path exists or update your profile",
            skills_path.display()
        );
    }

    let sync = SkillsLocalSync::new(Arc::clone(&db), skills_path.clone());
    let spinner = CliService::spinner("Calculating diff...");
    let diff = sync
        .calculate_diff()
        .await
        .context("Failed to calculate skills diff")?;
    spinner.finish_and_clear();

    display_diff_summary(&diff);

    if !diff.has_changes() {
        CliService::success("Skills are in sync - no changes needed");
        return Ok(CommandResult::text(SkillSyncOutput {
            direction: "none".to_string(),
            synced: 0,
            skipped: 0,
            deleted: 0,
            errors: vec![],
        })
        .with_title("Skills Sync"));
    }

    let direction = match args.direction {
        Some(SyncDirection::ToDisk) => LocalSyncDirection::ToDisk,
        Some(SyncDirection::ToDb) => LocalSyncDirection::ToDatabase,
        None => {
            if !config.is_interactive() {
                anyhow::bail!("--direction is required in non-interactive mode");
            }
            match prompt_sync_direction()? {
                Some(dir) => dir,
                None => {
                    CliService::info("Sync cancelled");
                    return Ok(CommandResult::text(SkillSyncOutput {
                        direction: "cancelled".to_string(),
                        synced: 0,
                        skipped: 0,
                        deleted: 0,
                        errors: vec![],
                    })
                    .with_title("Skills Sync"));
                }
            }
        }
    };

    if args.dry_run {
        CliService::info("[Dry Run] No changes made");
        let direction_str = match direction {
            LocalSyncDirection::ToDisk => "to-disk",
            LocalSyncDirection::ToDatabase => "to-db",
        };
        return Ok(CommandResult::text(SkillSyncOutput {
            direction: format!("{} (dry-run)", direction_str),
            synced: 0,
            skipped: 0,
            deleted: 0,
            errors: vec![],
        })
        .with_title("Skills Sync (Dry Run)"));
    }

    if !args.yes && config.is_interactive() {
        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Proceed with sync?")
            .default(false)
            .interact()
            .context("Failed to get confirmation")?;

        if !confirmed {
            CliService::info("Sync cancelled");
            return Ok(CommandResult::text(SkillSyncOutput {
                direction: "cancelled".to_string(),
                synced: 0,
                skipped: 0,
                deleted: 0,
                errors: vec![],
            })
            .with_title("Skills Sync"));
        }
    }

    let spinner = CliService::spinner("Syncing skills...");
    let result = match direction {
        LocalSyncDirection::ToDisk => sync.sync_to_disk(&diff, args.delete_orphans).await?,
        LocalSyncDirection::ToDatabase => sync.sync_to_db(&diff, args.delete_orphans).await?,
    };
    spinner.finish_and_clear();

    CliService::section("Sync Complete");
    CliService::key_value("Direction", &result.direction);
    CliService::key_value("Synced", &result.items_synced.to_string());
    CliService::key_value("Deleted", &result.items_deleted.to_string());
    CliService::key_value("Skipped", &result.items_skipped.to_string());

    if !result.errors.is_empty() {
        CliService::warning(&format!("Errors ({})", result.errors.len()));
        for error in &result.errors {
            CliService::error(&format!("  {}", error));
        }
    }

    let output = SkillSyncOutput {
        direction: result.direction,
        synced: result.items_synced,
        skipped: result.items_skipped,
        deleted: result.items_deleted,
        errors: result.errors,
    };

    Ok(CommandResult::text(output).with_title("Skills Sync"))
}

fn get_skills_path() -> Result<std::path::PathBuf> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.skills()))
}

async fn create_db_provider() -> Result<Arc<dyn DatabaseProvider>> {
    let url = SecretsBootstrap::database_url()
        .context("Database URL not configured")?
        .to_string();

    let database = Database::from_config("postgres", &url)
        .await
        .context("Failed to connect to database")?;

    Ok(Arc::new(database))
}

fn display_diff_summary(diff: &SkillsDiffResult) {
    CliService::section("Skills Status");
    CliService::info(&format!("{} unchanged", diff.unchanged));

    if !diff.added.is_empty() {
        CliService::info(&format!("+ {} (on disk, not in DB)", diff.added.len()));
        for item in &diff.added {
            let name = item.name.as_deref().unwrap_or("unnamed");
            CliService::info(&format!("    + {} ({})", item.skill_id, name));
        }
    }

    if !diff.removed.is_empty() {
        CliService::info(&format!("- {} (in DB, not on disk)", diff.removed.len()));
        for item in &diff.removed {
            let name = item.name.as_deref().unwrap_or("unnamed");
            CliService::info(&format!("    - {} ({})", item.skill_id, name));
        }
    }

    if !diff.modified.is_empty() {
        CliService::info(&format!("~ {} (modified)", diff.modified.len()));
        for item in &diff.modified {
            let name = item.name.as_deref().unwrap_or("unnamed");
            CliService::info(&format!("    ~ {} ({})", item.skill_id, name));
        }
    }
}

fn prompt_sync_direction() -> Result<Option<LocalSyncDirection>> {
    let options = vec![
        "Sync to database (Disk -> DB)",
        "Sync to disk (DB -> Disk)",
        "Cancel",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose sync direction")
        .items(&options)
        .default(0)
        .interact()
        .context("Failed to get direction selection")?;

    match selection {
        0 => Ok(Some(LocalSyncDirection::ToDatabase)),
        1 => Ok(Some(LocalSyncDirection::ToDisk)),
        _ => Ok(None),
    }
}
