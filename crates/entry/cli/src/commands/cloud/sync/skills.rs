use super::SkillsSyncArgs;
use crate::cli_settings::CliConfig;
use anyhow::{Context, Result};
use dialoguer::{Confirm, Select};
use std::sync::Arc;
use systemprompt_database::{Database, DbPool};
use systemprompt_logging::CliService;
use systemprompt_models::{AppPaths, SecretsBootstrap};
use systemprompt_sync::{LocalSyncDirection, LocalSyncResult, SkillsDiffResult, SkillsLocalSync};

fn get_skills_path() -> Result<std::path::PathBuf> {
    let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(paths.system().skills().to_path_buf())
}

async fn create_db_provider(database_url: Option<&str>) -> Result<DbPool> {
    let url = match database_url {
        Some(url) => url.to_string(),
        None => SecretsBootstrap::database_url()?.to_string(),
    };

    let database = Database::from_config("postgres", &url)
        .await
        .context("Failed to connect to database")?;

    Ok(Arc::new(database))
}

pub async fn execute(args: SkillsSyncArgs, config: &CliConfig) -> Result<()> {
    CliService::section("Skills Sync");

    let spinner = CliService::spinner("Connecting to database...");
    let db = create_db_provider(args.database_url.as_deref()).await?;
    spinner.finish_and_clear();

    let skills_path = get_skills_path()?;

    if !skills_path.exists() {
        anyhow::bail!(
            "Profile Error: Skills path does not exist\n\n  Path: {}\n  Field: paths.skills\n\n  \
             To fix: Ensure the path exists or update your profile",
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
        return Ok(());
    }

    let direction = match args.direction {
        Some(crate::cloud::sync::CliLocalSyncDirection::ToDisk) => LocalSyncDirection::ToDisk,
        Some(crate::cloud::sync::CliLocalSyncDirection::ToDb) => LocalSyncDirection::ToDatabase,
        None => {
            if !config.is_interactive() {
                anyhow::bail!("--direction is required in non-interactive mode");
            }
            if let Some(dir) = prompt_sync_direction()? {
                dir
            } else {
                CliService::info("Sync cancelled");
                return Ok(());
            }
        },
    };

    if args.dry_run {
        CliService::info("Dry run - no changes made");
        return Ok(());
    }

    if !args.yes && config.is_interactive() {
        let confirmed = Confirm::new()
            .with_prompt("Proceed with sync?")
            .default(false)
            .interact()?;

        if !confirmed {
            CliService::info("Sync cancelled");
            return Ok(());
        }
    }

    let spinner = CliService::spinner("Syncing skills...");
    let result = match direction {
        LocalSyncDirection::ToDisk => sync.sync_to_disk(&diff, args.delete_orphans).await?,
        LocalSyncDirection::ToDatabase => sync.sync_to_db(&diff, args.delete_orphans).await?,
    };
    spinner.finish_and_clear();

    display_sync_result(&result);

    Ok(())
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
        "Sync to disk (DB -> Disk)",
        "Sync to database (Disk -> DB)",
        "Cancel",
    ];

    let selection = Select::new()
        .with_prompt("Choose sync direction")
        .items(&options)
        .default(0)
        .interact()?;

    match selection {
        0 => Ok(Some(LocalSyncDirection::ToDisk)),
        1 => Ok(Some(LocalSyncDirection::ToDatabase)),
        _ => Ok(None),
    }
}

fn display_sync_result(result: &LocalSyncResult) {
    CliService::section("Sync Complete");
    CliService::key_value("Direction", &result.direction);
    CliService::key_value("Synced", &result.items_synced.to_string());
    CliService::key_value("Deleted", &result.items_deleted.to_string());
    CliService::key_value("Skipped", &result.items_skipped.to_string());

    if !result.errors.is_empty() {
        CliService::warning(&format!("Errors ({})", result.errors.len()));
        for error in &result.errors {
            CliService::error(&format!("    {}", error));
        }
    }
}
